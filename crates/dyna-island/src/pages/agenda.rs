//! Agenda page (the Outlook agenda add-on): what's left of today and
//! tomorrow's meetings, from the calendar link Outlook publishes. The meeting
//! under way or next carries the accent; all-day events come last in their day.
//!
//! With no link yet, the page says where Outlook.com gives one and offers
//! Paste link (`Command::AgendaPasteLink`); with one, its footer has Remove
//! link. The page only shows `st.agenda`; the app reads the link
//! (`services::agenda`) when it is set, every few minutes while someone is at
//! the PC, and when this page opens.

use crate::commands::Command;
use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::state::{AgendaStatus, AppState};
use crate::ui::{wid, PillStyle, PILL_REGULAR};
use dyna_core::agenda::{by_day, fmt_time, fmt_until, local_secs, next, Meeting};
use dyna_core::geom::{palette, Point, Rect};

const HEIGHT: f32 = 200.0;
const ROW_H: f32 = 34.0;
const LABEL_H: f32 = 20.0;
const TIME_W: f32 = 96.0;
const FOOTER_H: f32 = 16.0;

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

/// The end of a "that isn't a link" note.
pub fn next_wake(st: &AppState) -> Option<f64> {
    st.agenda.note.map(|(_, until)| until).filter(|t| *t > st.now)
}

pub fn draw(ui: &mut crate::ui::Ui, st: &mut AppState, area: Rect) {
    let now = local_secs(&st.local);
    if st.agenda.note.is_some_and(|(_, until)| st.now >= until) {
        st.agenda.note = None;
    }
    if st.settings.agenda_link.is_empty() {
        draw_no_link(ui, st, area);
        return;
    }
    let body = Rect::new(area.x, area.y, area.w, area.h - FOOTER_H);
    let footer = Rect::new(area.x, area.bottom() - FOOTER_H, area.w, FOOTER_H);
    let empty = st.agenda.meetings.is_empty();
    let shown = match &st.agenda.status {
        AgendaStatus::Idle | AgendaStatus::Reading if empty => Some(("Reading your calendar\u{2026}".to_string(), "Your meetings show here in a moment".to_string())),
        AgendaStatus::Offline(why) if empty => Some(("Couldn't reach Outlook".to_string(), why.clone())),
        AgendaStatus::Failed(why) if empty => Some(("Outlook didn't give the calendar".to_string(), why.clone())),
        _ => None,
    };
    if let Some((title, hint)) = shown {
        super::empty_state(ui, body, Icon::Agenda, &title, &hint);
        remove_link(ui, st, footer);
        return;
    }

    let use_24h = st.settings.use_24h;
    let accent = st.accent;
    let list = st.agenda.meetings.clone();
    let (today, tomorrow) = by_day(&list, now);
    let next_one = next(&list, now).map(|(m, _)| m.clone());
    // What's left of today first: meetings already over drop off.
    let left: Vec<&Meeting> = today.into_iter().filter(|m| m.all_day || m.end > now).collect();

    let mut y = body.y;
    let fits = |y: f32, h: f32| y + h <= body.bottom() + 0.5;
    label(ui, Rect::new(body.x, y, body.w, LABEL_H), "TODAY");
    y += LABEL_H;
    if left.is_empty() {
        ui.c.text("Nothing else today", Rect::new(body.x, y, body.w, ROW_H), &TextStyle::body().color(palette::TEXT_DIM));
        y += ROW_H;
    }
    for m in &left {
        if !fits(y, ROW_H) {
            break;
        }
        let is_next = next_one.as_ref() == Some(*m);
        row(ui, Rect::new(body.x, y, body.w, ROW_H), m, use_24h, if is_next { Some((accent, now)) } else { None });
        y += ROW_H;
    }
    if !tomorrow.is_empty() && fits(y + 6.0, LABEL_H + ROW_H) {
        y += 6.0;
        label(ui, Rect::new(body.x, y, body.w, LABEL_H), "TOMORROW");
        y += LABEL_H;
        for m in &tomorrow {
            if !fits(y, ROW_H) {
                break;
            }
            row(ui, Rect::new(body.x, y, body.w, ROW_H), m, use_24h, None);
            y += ROW_H;
        }
    }

    // When the calendar was last read (or that the last try didn't get it).
    let note = match (&st.agenda.status, st.agenda.read_at.as_deref()) {
        (AgendaStatus::Offline(_), Some(t)) => format!("Offline \u{b7} from Outlook at {t}"),
        (AgendaStatus::Failed(_), _) => "Outlook didn't give the calendar the last time".to_string(),
        (_, Some(t)) => format!("From Outlook at {t}"),
        _ => String::new(),
    };
    ui.c.text(&note, footer, &TextStyle::caption().color(palette::TEXT_FAINT));
    remove_link(ui, st, footer);
}

/// No link yet: where Outlook.com gives one, and Paste link.
fn draw_no_link(ui: &mut crate::ui::Ui, st: &mut AppState, area: Rect) {
    let top = Rect::new(area.x, area.y, area.w, area.h - 64.0);
    super::empty_state(
        ui,
        top,
        Icon::Agenda,
        "Link your Outlook calendar",
        "Outlook.com: Settings \u{203a} Calendar \u{203a} Shared calendars \u{203a} Publish, copy the ICS link",
    );
    let label = "Paste link";
    let w = ui.pill_width(label, true, false);
    let r = Rect::new(area.center().x - w / 2.0, top.bottom() + 2.0, w, PILL_REGULAR);
    if ui.pill(wid("agenda.paste"), r, label, Some(Icon::Clipboard), PillStyle::Primary(st.accent)).clicked {
        st.cmd(Command::AgendaPasteLink);
    }
    let note = st.agenda.note.map(|(n, _)| n).unwrap_or("Only this link is ever read, and only while this add-on is on.");
    let color = if st.agenda.note.is_some() { palette::ORANGE } else { palette::TEXT_FAINT };
    ui.c.text(note, Rect::new(area.x, r.bottom() + 6.0, area.w, 16.0), &TextStyle::caption().color(color).center());
}

/// "Remove link" at the right of the footer: forget the calendar.
fn remove_link(ui: &mut crate::ui::Ui, st: &mut AppState, footer: Rect) {
    let label = "Remove link";
    let style = TextStyle::caption();
    let w = ui.c.measure(label, &style, 200.0).0.ceil() + 4.0;
    let r = Rect::new(footer.right() - w, footer.y, w, footer.h);
    let resp = ui.button(wid("agenda.remove"), r);
    let color = if resp.hovered { palette::TEXT } else { palette::TEXT_FAINT };
    ui.c.text(label, r, &style.color(color).right());
    if resp.clicked {
        st.cmd(Command::AgendaRemoveLink);
    }
}

fn label(ui: &mut crate::ui::Ui, r: Rect, text: &str) {
    ui.c.text(text, r, &TextStyle::caption().weight(Weight::Semibold).color(palette::TEXT_FAINT));
}

/// One meeting: its times on the left, the subject over the place. `next`:
/// the accent and the island's clock, for the one under way or next.
fn row(ui: &mut crate::ui::Ui, r: Rect, m: &Meeting, use_24h: bool, next: Option<(dyna_core::geom::Color, i64)>) {
    let times = if m.all_day { "All day".to_string() } else { format!("{}\u{2013}{}", fmt_time(m.start, use_24h), fmt_time(m.end, use_24h)) };
    let time_st = TextStyle::footnote().tabular().color(if next.is_some() { palette::TEXT } else { palette::TEXT_DIM });
    if let Some((accent, _)) = next {
        ui.c.fill_circle(Point::new(r.x + 3.0, r.center().y), 3.0, accent);
    }
    ui.c.text(&times, Rect::new(r.x + 12.0, r.y, TIME_W, r.h), &time_st);
    let x = r.x + 12.0 + TIME_W;
    let mut right = r.right();
    if let Some((accent, now)) = next {
        let until = fmt_until(m.start - now);
        let st = TextStyle::footnote().weight(Weight::Semibold).color(accent).right();
        let w = ui.c.measure(&until, &st, 200.0).0.ceil();
        ui.c.text(&until, Rect::new(r.right() - w - 2.0, r.y, w + 2.0, r.h), &st);
        right -= w + 12.0;
    }
    let w = (right - x).max(0.0);
    let subject = if m.subject.is_empty() { "(no subject)" } else { m.subject.as_str() };
    if m.location.is_empty() {
        ui.c.text(subject, Rect::new(x, r.y, w, r.h), &TextStyle::body());
    } else {
        ui.c.text(subject, Rect::new(x, r.y + 1.0, w, 17.0), &TextStyle::body());
        ui.c.text(&m.location, Rect::new(x, r.y + 17.0, w, 15.0), &TextStyle::caption().color(palette::TEXT_DIM));
    }
}
