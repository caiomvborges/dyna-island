//! The Light theme's wave: a small great wave, after Hokusai's print, in
//! the places the island has room for a picture (beside the clock, as the
//! idle look, in the music pill and beside the pin on the pages).
//!
//! It is drawn from a few vector shapes in a 100 x 64 box whose bottom edge
//! sits on a line near the island's bottom: a Prussian-blue body that rises
//! on the left and curls over to the right, two paler bands along it, a
//! white crest with claws hanging into the hollow, spray, and Fuji far off
//! in the hollow.
//!
//! It moves like water: a ripple runs along it, the crest leans over and
//! back, the swell lifts it and the spray flies up off the curl. In the music
//! pill it moves for as long as something plays (that pill already redraws
//! every frame for the equalizer it replaces). Elsewhere it rolls for
//! [`ROLL_LEN`] seconds every [`ROLL_EVERY`] and stands still in between, so
//! the island sleeps most of the time; the callers skip the rolls on battery
//! saver, and ambient frames already stop while the PC is locked or the
//! display is off.

use crate::gfx::Canvas;
use poni_core::geom::{Color, Point, Rect};
use poni_core::settings::Theme;
use poni_core::svgpath::{self, Seg};
use std::f64::consts::TAU;
use std::sync::OnceLock;

pub const DEEP: Color = Color::hex(0x1F4E8C);
const MID: Color = Color::hex(0x5B8DC4);
const PALE: Color = Color::hex(0xB9D2EA);
const FOAM: Color = Color::hex(0xFFFFFF);

const BOX_W: f32 = 100.0;
const BOX_H: f32 = 64.0;

const BODY: &str = "M0 64C10 60 22 46 30 32C38 16 52 4 68 4C82 4 94 12 97 23C91 17 81 16 75 21C67 28 67 41 75 51C81 58 90 62 100 64Z";
const BAND_MID: &str = "M12 64C20 56 28 44 36 30C44 17 54 10 64 9C54 15 46 25 40 36C34 47 28 57 26 64Z";
const BAND_PALE: &str = "M32 64C38 54 45 42 53 31C58 24 63 19 70 17C63 25 58 33 54 42C50 51 47 58 45 64Z";
const CREST: &str = "M30 32C38 16 52 4 68 4C82 4 94 12 97 23L93 21L94 28L89 22L88 29L84 22L81 28L79 21C70 14 58 13 48 20C42 24 37 29 33 35Z";
const FUJI: &str = "M76 61L86 49L97 61Z";
const SNOW: &str = "M83.4 52.1L86 49L88.8 52.4L87.2 51.6L86 52.9L84.8 51.5Z";
/// Spray in the hollow under the curl: centre and radius in box units.
const SPRAY: [(f32, f32, f32); 4] = [(86.0, 34.0, 1.8), (92.0, 38.0, 1.4), (81.0, 40.0, 1.2), (95.5, 31.0, 1.3)];

/// How long one sway of the crest takes, one rise and fall of the swell,
/// and one ripple along the wave (its length in box units).
const SWAY: f64 = 1.6;
const SWELL: f64 = 0.8;
const RIPPLE: f64 = 1.3;
const RIPPLE_LEN: f64 = 45.0;
/// How long a puff of spray takes to rise and fade.
const SPRAY_LIFE: f64 = 1.1;

/// Away from the music, the wave rolls this long (seconds)...
pub const ROLL_LEN: f64 = 3.0;
/// ...once in this many: a little life now and then, while costing frames
/// only three seconds in eight.
pub const ROLL_EVERY: f64 = 8.0;

/// How much the wave moves at `now` away from the music (0 still, 1 full):
/// it eases in and out of each roll.
pub fn roll(now: f64) -> f32 {
    let p = now.rem_euclid(ROLL_EVERY);
    if p >= ROLL_LEN {
        return 0.0;
    }
    let s = (std::f64::consts::PI * p / ROLL_LEN).sin();
    (s * s) as f32
}

/// Is a roll under way at `now`?
pub fn rolling(now: f64) -> bool {
    now.rem_euclid(ROLL_EVERY) < ROLL_LEN
}

/// When the next roll starts.
pub fn next_roll(now: f64) -> f64 {
    now - now.rem_euclid(ROLL_EVERY) + ROLL_EVERY
}

pub fn on(theme: Theme) -> bool {
    theme == Theme::Light
}

/// The wave's height for a width of `w` DIPs.
pub fn height(w: f32) -> f32 {
    w * BOX_H / BOX_W
}

struct Shapes {
    body: Vec<Seg>,
    band_mid: Vec<Seg>,
    band_pale: Vec<Seg>,
    crest: Vec<Seg>,
    fuji: Vec<Seg>,
    snow: Vec<Seg>,
}

fn shapes() -> &'static Shapes {
    static S: OnceLock<Shapes> = OnceLock::new();
    S.get_or_init(|| {
        let p = |d: &str| svgpath::parse(d).unwrap_or_default();
        Shapes { body: p(BODY), band_mid: p(BAND_MID), band_pale: p(BAND_PALE), crest: p(CREST), fuji: p(FUJI), snow: p(SNOW) }
    })
}

/// The wave `w` DIPs wide, centred on `cx`, its foot on `base_y`, inside
/// `clip`, at the time `now`. `amp` is how much it moves: 0 stands it
/// still, 1 is the full motion (see [`roll`]).
pub fn draw(c: &mut Canvas, cx: f32, base_y: f32, w: f32, clip: Rect, now: f64, amp: f32) {
    let k = w / BOX_W;
    let amp = amp.clamp(0.0, 1.0);
    let sway = amp * (TAU * now / SWAY).sin() as f32;
    let swell = amp * (0.5 + 0.5 * (TAU * now / SWELL).sin()) as f32;
    let to_canvas = |x: f32, y: f32| (cx + (x - BOX_W / 2.0) * k, base_y + (y - BOX_H) * k);
    // A ripple runs along the water, stronger towards the crest; the higher
    // a point, the further it leans and lifts with the swell.
    let map = |x: f32, y: f32| {
        let up = (BOX_H - y) / BOX_H;
        let ripple = amp * (TAU * (x as f64 / RIPPLE_LEN - now / RIPPLE)).sin() as f32;
        let x = x + sway * 5.0 * up * up;
        let y = y - swell * 3.0 * up - ripple * 2.2 * (0.35 + 0.65 * up);
        to_canvas(x, y)
    };
    let s = shapes();
    let stroke = (0.9 * k).max(0.6);
    c.clip(clip);
    c.unthemed(|c| {
        // Fuji is far off: it keeps still while the water moves.
        if w >= 34.0 {
            c.path(&place(&s.fuji, &to_canvas), Some(MID), None);
            c.path(&place(&s.snow, &to_canvas), Some(FOAM), None);
        }
        c.path(&place(&s.body, &map), Some(DEEP), None);
        c.path(&place(&s.band_mid, &map), Some(MID), None);
        c.path(&place(&s.band_pale, &map), Some(PALE), None);
        c.path(&place(&s.crest, &map), Some(FOAM), Some((DEEP, stroke)));
        // Spray: still drops at rest; while it moves, each drop flies up
        // and out from under the curl and fades, one after another.
        for (i, (x, y, r)) in SPRAY.into_iter().enumerate() {
            if amp <= 0.0 {
                let (px, py) = map(x, y);
                c.fill_circle(Point::new(px, py), (r * k).max(0.8), PALE);
                continue;
            }
            let p = (now / SPRAY_LIFE + i as f64 * 0.27).rem_euclid(1.0) as f32;
            let (px, py) = map(x + 3.0 * p, y - 7.0 * p);
            let alpha = (amp * (1.0 - p) * 1.4).min(1.0);
            c.with_alpha(alpha, |c| c.fill_circle(Point::new(px, py), (r * k * (1.0 - 0.4 * p)).max(0.7), PALE));
        }
    });
    c.unclip();
}

/// `segs` with every point moved through `map`.
fn place(segs: &[Seg], map: &impl Fn(f32, f32) -> (f32, f32)) -> Vec<Seg> {
    segs.iter()
        .map(|s| match *s {
            Seg::MoveTo(x, y) => {
                let (x, y) = map(x, y);
                Seg::MoveTo(x, y)
            }
            Seg::LineTo(x, y) => {
                let (x, y) = map(x, y);
                Seg::LineTo(x, y)
            }
            Seg::QuadTo(x1, y1, x, y) => {
                let ((x1, y1), (x, y)) = (map(x1, y1), map(x, y));
                Seg::QuadTo(x1, y1, x, y)
            }
            Seg::CubicTo(x1, y1, x2, y2, x, y) => {
                let ((x1, y1), (x2, y2), (x, y)) = (map(x1, y1), map(x2, y2), map(x, y));
                Seg::CubicTo(x1, y1, x2, y2, x, y)
            }
            Seg::ArcTo(rx, ry, rot, large, sweep, x, y) => {
                let (x, y) = map(x, y);
                Seg::ArcTo(rx, ry, rot, large, sweep, x, y)
            }
            Seg::Close => Seg::Close,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wave_shapes_parse() {
        for d in [BODY, BAND_MID, BAND_PALE, CREST, FUJI, SNOW] {
            let segs = svgpath::parse(d).expect(d);
            assert!(segs.len() > 2, "{d}");
        }
    }

    #[test]
    fn it_rolls_now_and_then_and_rests_between() {
        assert_eq!(roll(0.0), 0.0, "a roll eases in");
        assert!(roll(ROLL_LEN / 2.0) > 0.99, "full motion mid-roll");
        assert_eq!(roll(ROLL_LEN + 0.1), 0.0);
        assert!(rolling(1.0) && !rolling(ROLL_LEN + 0.1));
        assert_eq!(next_roll(ROLL_LEN + 1.0), ROLL_EVERY);
        assert_eq!(next_roll(ROLL_EVERY), 2.0 * ROLL_EVERY);
        assert!(ROLL_LEN < ROLL_EVERY);
    }

    #[test]
    fn it_keeps_to_its_box() {
        for d in [BODY, BAND_MID, BAND_PALE, CREST, FUJI, SNOW] {
            for s in svgpath::parse(d).unwrap() {
                let pts: Vec<(f32, f32)> = match s {
                    Seg::MoveTo(x, y) | Seg::LineTo(x, y) => vec![(x, y)],
                    Seg::QuadTo(a, b, x, y) => vec![(a, b), (x, y)],
                    Seg::CubicTo(a, b, c, d, x, y) => vec![(a, b), (c, d), (x, y)],
                    _ => vec![],
                };
                for (x, y) in pts {
                    assert!((0.0..=BOX_W).contains(&x) && (0.0..=BOX_H).contains(&y), "{d}: {x},{y}");
                }
            }
        }
    }
}
