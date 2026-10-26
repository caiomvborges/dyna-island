//! Music page, the flagship screen: album art, track info, a seekable
//! progress bar and transport controls.
//!
//! ```text
//! ┌──────────┐  Title
//! │          │  Artist
//! │   art    │  • Player
//! │ 104×104  │  ━━━━━━━━━━━━━━━━━──────────
//! └──────────┘  1:23                 −2:10
//!                  ⏮      ( ▶ )      ⏭
//! ```
//!
//! The app lays pages out at the island's final size (and masks them with
//! the island's outline while it springs open), so nothing reflows here.
//! The art's top edge and the title's cap height share the content top.
//! All units are DIPs; static geometry is snapped to device pixels.
//!
//! Animation state that has no home in `PageUiState` (velocities, the last
//! frame time) lives in a UI-thread `thread_local`.

use crate::commands::{Command, MediaCmd};
use crate::gfx::TextStyle;
use crate::icons::Icon;
use crate::state::{AppState, MediaSession};
use crate::ui::{wid, Ui, WidgetId};
use poni_core::datetime::fmt_duration;
use poni_core::geom::{palette, Color, Point, Rect};
use std::cell::Cell;

// ---------------------------------------------------------------- layout

/// Content height with a media session (the play button's bottom + air).
const H_SESSION: f32 = 148.0;
/// Content height of the "Nothing playing" state.
const H_EMPTY: f32 = 124.0;

const ART: f32 = 104.0;
const ART_RADIUS: f32 = 14.0;
/// The art's first ink is the content top, like every page.
const ART_TOP: f32 = 0.0;
/// Art shrinks a little while paused (animated).
pub(crate) const ART_PAUSED_SCALE: f32 = 0.94;
const GAP_ART: f32 = 16.0;

// Middle column rows (offsets from the content top). The title's text box
// starts above the top so its cap height (not its line box) meets the art's
// top edge; the header leaves room for that. Title, artist and player sit on
// a ~20 DIP rhythm; the bar + times group has about the same air (~14 DIPs
// of ink gap) above it and above the controls.
const TITLE_Y: f32 = -6.0;
const TITLE_H: f32 = 22.0;
const ARTIST_Y: f32 = 16.0;
const SOURCE_Y: f32 = 37.0;
const SOURCE_H: f32 = 18.0;
const BAR_Y: f32 = 66.0;
const BAR_H: f32 = 4.0;
const TIMES_Y: f32 = 75.0;
const TIMES_H: f32 = 14.0;
const CONTROLS_CY: f32 = 122.0;

const PLAY_D: f32 = 44.0;
const SIDE_BTN: f32 = 38.0;
const CONTROL_ICON: f32 = 20.0;

// Critically damped follow rate (rad/s): settle time is roughly 4/ω.
pub(crate) const ART_OMEGA: f32 = 14.0;
/// A motion that starts on a slow ambient frame (the clock runs near 30 fps
/// at rest) first moves this much time, not the whole gap since the last
/// frame, so it doesn't open with a jump.
const START_STEP_S: f32 = 1.0 / 120.0;

/// A gap this long since the page last drew means it was hidden meanwhile.
const RESUME_GAP_S: f64 = 0.5;
const MAX_DT_S: f64 = 0.25;

// ---------------------------------------------------------------- animation state

#[derive(Default)]
pub(crate) struct Anim {
    last_t: Option<f64>,
    /// Current art scale (0 = not initialised yet).
    pub(crate) art_scale: f32,
    pub(crate) art_vel: f32,
}

pub(crate) struct Frame {
    pub(crate) dt: f32,
    /// Resumed while the island is still fading its content in.
    pub(crate) opening: bool,
}

impl Anim {
    pub(crate) fn frame(&mut self, now: f64, content_alpha: f32) -> Frame {
        let gap = self.last_t.map_or(f64::INFINITY, |t| now - t);
        self.last_t = Some(now);
        let resumed = !(0.0..=RESUME_GAP_S).contains(&gap);
        let dt = if (0.0..=MAX_DT_S).contains(&gap) { gap as f32 } else { 1.0 / 60.0 };
        Frame { dt, opening: resumed && content_alpha < 0.98 }
    }
}

thread_local! {
    // Pages only ever run on the UI thread.
    static ANIM: Cell<Option<Box<Anim>>> = const { Cell::new(None) };
}

pub(crate) fn take_anim() -> Box<Anim> {
    ANIM.try_with(|c| c.take()).ok().flatten().unwrap_or_default()
}

pub(crate) fn put_anim(a: Box<Anim>) {
    let _ = ANIM.try_with(|c| c.set(Some(a)));
}

/// Exact critically damped spring step toward `target` (never overshoots,
/// stable for any `dt`).
pub(crate) fn follow(pos: &mut f32, vel: &mut f32, target: f32, omega: f32, dt: f32) {
    if !pos.is_finite() || !vel.is_finite() {
        *pos = target;
        *vel = 0.0;
        return;
    }
    let x = *pos - target;
    let e = (-omega * dt).exp();
    let k = (*vel + omega * x) * dt;
    *pos = target + (x + k) * e;
    *vel = (*vel - omega * k) * e;
}

/// Time to step a follow this frame. A motion that starts from rest moves
/// at most [`START_STEP_S`], whatever the gap since the last (maybe ambient)
/// frame; one already under way keeps the real gap.
pub(crate) fn step_dt(dt: f32, vel: f32, pos: f32, target: f32) -> f32 {
    if vel == 0.0 && pos != target {
        dt.min(START_STEP_S)
    } else {
        dt
    }
}

/// Snap to the target once close and slow enough; returns `true` when settled.
pub(crate) fn settle(pos: &mut f32, vel: &mut f32, target: f32, eps: f32, vel_eps: f32) -> bool {
    if (*pos - target).abs() < eps && vel.abs() < vel_eps {
        *pos = target;
        *vel = 0.0;
        true
    } else {
        false
    }
}

pub(crate) fn snap(v: f32, scale: f32) -> f32 {
    (v * scale).round() / scale
}

// ---------------------------------------------------------------- page

pub fn height(st: &AppState) -> f32 {
    if let Some(h) = crate::pill::music::height(st) {
        return h;
    }
    if st.media.is_some() {
        H_SESSION
    } else {
        H_EMPTY
    }
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    if crate::pill::music::draw(ui, st, area) {
        return;
    }
    let mut a = take_anim();
    let fr = a.frame(st.now, ui.c.alpha());
    if st.media.is_some() {
        draw_session(ui, st, area, &mut a, &fr);
    } else {
        let r = Rect::new(area.x, area.y, area.w, H_EMPTY);
        super::empty_state(ui, r, Icon::Music, "Nothing playing", "Play music in Spotify, a browser or any media app");
    }
    put_anim(a);
}

fn draw_session(ui: &mut Ui, st: &mut AppState, area: Rect, a: &mut Anim, fr: &Frame) {
    let now = st.now;
    let accent = st.media_accent();
    let Some(m) = st.media.as_ref() else { return };

    let scale = ui.c.scale().max(0.5);
    let w = area.w;
    let x0 = snap(area.x, scale);
    let y0 = snap(area.y, scale);

    // The middle column, right of the art.
    let mid_x = snap(x0 + ART + GAP_ART, scale);
    let mid = Rect::new(mid_x, y0, snap(x0 + w - mid_x, scale).max(0.0), H_SESSION);

    let playing = m.is_playing();
    let tl = m.timeline;
    let pos = tl.position_at(now);
    let has_dur = tl.has_duration();
    let mut acts: Vec<MediaCmd> = Vec::new();
    let mut anim = false;

    // Album art (gently shrinks while paused).
    let art_target = if playing { 1.0 } else { ART_PAUSED_SCALE };
    if a.art_scale <= 0.0 || fr.opening {
        a.art_scale = art_target;
        a.art_vel = 0.0;
    }
    let dt = step_dt(fr.dt, a.art_vel, a.art_scale, art_target);
    follow(&mut a.art_scale, &mut a.art_vel, art_target, ART_OMEGA, dt);
    let art_settled = settle(&mut a.art_scale, &mut a.art_vel, art_target, 0.0005, 0.005);
    anim |= !art_settled;
    let art_px = if art_settled { Some(scale) } else { None };
    draw_art(ui, m, Rect::new(x0, y0 + ART_TOP, ART, ART), a.art_scale, art_px);

    // Title, artist, player.
    let title = match m.title.trim() {
        "" => "Unknown title",
        t => t,
    };
    let artist = match m.artist.trim() {
        "" => m.album.trim(),
        t => t,
    };
    ui.c.text(title, Rect::new(mid.x, y0 + TITLE_Y, mid.w, TITLE_H), &TextStyle::headline());
    if !artist.is_empty() {
        ui.c.text(artist, Rect::new(mid.x, y0 + ARTIST_Y, mid.w, 18.0), &TextStyle::body().color(palette::TEXT_DIM));
    }
    let app = m.app_name.trim();
    let source = match (app.is_empty(), playing) {
        (false, true) => Some(app.to_string()),
        (false, false) => Some(format!("{app} \u{b7} Paused")),
        (true, false) => Some("Paused".to_string()),
        (true, true) => None,
    };
    let mut open_player = false;
    if let Some(label) = source {
        let dot = if playing { accent } else { accent.mul_alpha(0.45) };
        open_player = draw_source(ui, mid.x, y0 + SOURCE_Y, mid.w, &label, dot, scale);
    }

    // Progress + times.
    let bar = Rect::new(mid.x, snap(y0 + BAR_Y, scale), mid.w, BAR_H);
    if has_dur {
        let seekable = m.can_seek;
        let hover = seekable && ui.hovered(Rect::new(bar.x, bar.y - 6.0, bar.w, bar.h + 12.0));
        let scrub = ui.progress_bar(wid("media_seek"), bar, tl.progress_at(now), accent, seekable);
        if scrub.began {
            // A drag starts: the player can get ready for the seek meanwhile.
            acts.push(MediaCmd::Prepare);
        }
        if let Some(f) = scrub.released {
            acts.push(MediaCmd::Seek(f as f64 * tl.duration_s));
        }
        // While dragging, the knob and times follow the pointer; the player
        // only seeks on release.
        let (progress, pos) = match scrub.held {
            Some(f) => (f, f as f64 * tl.duration_s),
            None => (tl.progress_at(now), pos),
        };
        if hover || scrub.held.is_some() {
            let c = Point::new(bar.x + bar.w * progress, bar.y + bar.h / 2.0);
            ui.c.fill_circle(c, 6.0, palette::TEXT);
        }
        let ts = TextStyle::footnote().tabular().color(palette::TEXT_DIM);
        let half = mid.w / 2.0;
        let remaining = (tl.duration_s.floor() - pos.floor()).max(0.0);
        ui.c.text(&fmt_duration(pos), Rect::new(mid.x, y0 + TIMES_Y, half, TIMES_H), &ts);
        ui.c.text(&format!("\u{2212}{}", fmt_duration(remaining)), Rect::new(mid.x + half, y0 + TIMES_Y, half, TIMES_H), &ts.right());
    } else {
        // Unknown length (streams, some browsers): a quiet, inert track.
        ui.c.fill_rrect(bar, BAR_H / 2.0, palette::TRACK.mul_alpha(0.6));
    }

    // Transport controls.
    let cx = mid.x + mid.w / 2.0;
    let cy = y0 + CONTROLS_CY;
    let spacing = ((mid.w - SIDE_BTN) / 2.0).clamp(40.0, 60.0);
    if side_button(ui, wid("media_prev"), Point::new(cx - spacing, cy), Icon::Prev, m.can_prev) {
        acts.push(MediaCmd::Prev);
    }
    if play_button(ui, Point::new(cx, cy), playing, m.can_play_pause) {
        acts.push(MediaCmd::PlayPause);
    }
    if side_button(ui, wid("media_next"), Point::new(cx + spacing, cy), Icon::Next, m.can_next) {
        acts.push(MediaCmd::Next);
    }

    // Real transitions (the art's spring, a press) get full-rate frames; the
    // steady drift of the progress bar while music plays only needs ambient
    // ~30 fps.
    if anim {
        ui.request_anim();
    } else if playing && has_dur {
        ui.request_live();
    }

    for c in acts {
        push_media(st, c);
    }
    if open_player {
        st.cmd(Command::OpenPlayer);
    }
}

/// Queue a media command; seeks also move the local timeline right away so
/// the bar doesn't wait for the player to report back.
pub(crate) fn push_media(st: &mut AppState, c: MediaCmd) {
    let c = match c {
        MediaCmd::Seek(t) => {
            let now = st.now;
            let mut t = if t.is_finite() { t.max(0.0) } else { 0.0 };
            if let Some(ms) = st.media.as_mut() {
                if ms.timeline.duration_s > 0.0 {
                    t = t.min(ms.timeline.duration_s);
                }
                ms.timeline.position_s = t;
                ms.timeline.sampled_at = now;
            }
            MediaCmd::Seek(t)
        }
        other => other,
    };
    st.media_cmd(c);
}

/// Cover art (or a placeholder tile) scaled about its centre by `scale`;
/// snapped to device pixels when `px_scale` is given (i.e. at rest).
fn draw_art(ui: &mut Ui, m: &MediaSession, r: Rect, scale: f32, px_scale: Option<f32>) {
    let mut rr = Rect::from_center(r.center(), r.w * scale, r.h * scale);
    if let Some(s) = px_scale {
        let (x0, y0) = (snap(rr.x, s), snap(rr.y, s));
        rr = Rect::new(x0, y0, snap(rr.right(), s) - x0, snap(rr.bottom(), s) - y0);
    }
    let radius = ART_RADIUS * scale;
    match &m.art {
        Some(img) => {
            ui.c.image(img, rr, radius);
            // Hairline so dark covers keep an edge on the black island.
            ui.c.stroke_rrect(rr, radius, Color::white(0.08), 1.0);
        }
        None => {
            ui.c.fill_rrect(rr, radius, palette::SURFACE);
            let s = 40.0 * scale;
            ui.c.icon(Icon::Music, Rect::from_center(rr.center(), s, s), palette::TEXT_FAINT);
        }
    }
}

/// The player's name after a small accent dot, as plain secondary text.
/// The player line ("• Spotify · Paused"). It is a button: a click brings
/// the player forward (`Command::OpenPlayer`), and under the pointer it
/// brightens with an arrow after it. Returns whether it was clicked.
fn draw_source(ui: &mut Ui, x: f32, y: f32, max_w: f32, label: &str, dot: Color, scale: f32) -> bool {
    let (dot_r, gap, arrow) = (3.0, 6.0, 12.0);
    let tx = x + 2.0 * dot_r + gap;
    let tw = (x + max_w - tx - arrow - 4.0).max(0.0);
    if tw < 24.0 {
        return false;
    }
    let base = TextStyle::footnote();
    let text_w = ui.c.measure(label, &base, tw).0.ceil().min(tw);
    let hit = Rect::new(x - 4.0, y, (tx - x) + text_w + arrow + 10.0, SOURCE_H);
    let resp = ui.button(wid("media_player"), hit);
    let ts = base.color(if resp.hovered { palette::TEXT } else { palette::TEXT_DIM });
    ui.c.fill_circle(Point::new(x + dot_r, snap(y + SOURCE_H / 2.0, scale)), dot_r, dot);
    ui.c.text(label, Rect::new(tx, y, tw, SOURCE_H), &ts);
    if resp.hovered {
        ui.c.icon(Icon::Open, Rect::new(tx + text_w + 4.0, y + (SOURCE_H - arrow) / 2.0, arrow, arrow), palette::TEXT_DIM);
    }
    resp.clicked
}

/// Previous / next. Disabled buttons are drawn inert and take no input.
fn side_button(ui: &mut Ui, id: WidgetId, c: Point, icon: Icon, enabled: bool) -> bool {
    let r = Rect::from_center(c, SIDE_BTN, SIDE_BTN);
    if enabled {
        ui.icon_button(id, r, icon, palette::TEXT, CONTROL_ICON).clicked
    } else {
        ui.c.icon(icon, Rect::from_center(c, CONTROL_ICON, CONTROL_ICON), palette::DISABLED);
        false
    }
}

/// The big play/pause: a white disc with a black glyph (on a light look, its
/// accent disc with a white glyph).
fn play_button(ui: &mut Ui, c: Point, playing: bool, enabled: bool) -> bool {
    let icon = if playing { Icon::Pause } else { Icon::Play };
    let icon_r = Rect::from_center(c, CONTROL_ICON, CONTROL_ICON);
    if !enabled {
        ui.c.fill_circle(c, PLAY_D / 2.0, palette::SURFACE);
        ui.c.icon(icon, icon_r, palette::DISABLED);
        return false;
    }
    let resp = ui.button(wid("media_play"), Rect::from_center(c, PLAY_D, PLAY_D));
    // A look with its own accent (Light's amber, or Pill's) plays in it; the others in white.
    let (rest, hover, press, glyph) = match ui.c.look().accent {
        Some(a) => (a, Color::lerp(a, Color::white(1.0), 0.08), Color::lerp(a, Color::black(1.0), 0.15), palette::ON_ACCENT),
        None => (palette::TEXT, Color::white(1.0), Color::white(0.82), Color::black(1.0)),
    };
    let (radius, fill) = if resp.pressed {
        (PLAY_D / 2.0 - 1.5, press)
    } else if resp.hovered {
        (PLAY_D / 2.0 + 1.0, hover)
    } else {
        (PLAY_D / 2.0, rest)
    };
    ui.c.fill_circle(c, radius, fill);
    ui.c.icon(icon, icon_r, glyph);
    resp.clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scroll_from_rest_opens_with_one_short_step() {
        // An ambient frame 28 ms after the last one: the new scroll moves 1/120 s.
        assert_eq!(step_dt(0.028, 0.0, 2.0, 3.0), START_STEP_S);
        // Already moving, or nothing to do: the real gap.
        assert_eq!(step_dt(0.028, 0.4, 2.3, 3.0), 0.028);
        assert_eq!(step_dt(0.028, 0.0, 3.0, 3.0), 0.028);
        // A short gap stays as it is.
        assert_eq!(step_dt(0.004, 0.0, 2.0, 3.0), 0.004);
    }

    #[test]
    fn follow_converges_without_overshoot() {
        let (mut p, mut v) = (0.0f32, 0.0f32);
        let mut max = 0.0f32;
        for _ in 0..120 {
            follow(&mut p, &mut v, 5.0, ART_OMEGA, 1.0 / 60.0);
            max = max.max(p);
        }
        assert!((p - 5.0).abs() < 0.01);
        assert!(max <= 5.0 + 1e-4);
    }

    #[test]
    fn follow_is_stable_for_big_steps() {
        let (mut p, mut v) = (10.0f32, 0.0f32);
        follow(&mut p, &mut v, 0.0, ART_OMEGA, 5.0);
        assert!(p.is_finite() && p.abs() < 0.01);
    }
}
