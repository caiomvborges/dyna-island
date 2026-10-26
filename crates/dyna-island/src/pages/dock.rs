//! Dock page: the programs and shortcuts the person picked, as tiles (the
//! app's own icon over its name). A click opens the app and closes the
//! island; the × that shows on a hovered tile takes it off the Dock (the file
//! itself stays). The last tile, Add, opens Windows' Open dialog
//! (`Command::PickDockApp`); with nothing on the Dock yet, the page says what
//! it is for and offers the same button.
//!
//! The list lives in `data.json` (`dyna_core::dock`). Icons come from
//! Windows on the thumbnail worker the first time a tile draws
//! (`Command::LoadDockIcon`, kept in `AppState::dock_icons`), so the page
//! never touches the file system itself.

use crate::commands::Command;
use crate::gfx::{Image, TextStyle};
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid, wid_n, PillStyle, Ui, PILL_REGULAR};
use dyna_core::dock::DockApp;
use dyna_core::geom::{palette, Color, Point, Rect};

/// Two rows of tiles.
const TILE_H: f32 = 76.0;
const ROW_GAP: f32 = 8.0;
const HEIGHT: f32 = 2.0 * TILE_H + ROW_GAP;
/// A tile is at least this wide; the row shares what's left evenly.
const TILE_MIN_W: f32 = 72.0;
const TILE_R: f32 = 12.0;
/// The app's icon, and the share of its picture the icon fills (the
/// thumbnail worker leaves air around icons, as on the System page).
const ICON: f32 = 34.0;
const ICON_FILL: f32 = 0.72;
const ICON_TOP: f32 = 9.0;
const NAME_TOP: f32 = 50.0;
const NAME_H: f32 = 16.0;
/// The remove button in a hovered tile's top-right corner.
const REMOVE_D: f32 = 18.0;

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

/// Pixels to keep an icon at for this DPI scale.
pub fn icon_px(scale: f32) -> u32 {
    (ICON / ICON_FILL * scale.max(1.0)).ceil() as u32
}

/// Columns of tiles in a page `w` wide.
fn columns(w: f32) -> usize {
    ((w / TILE_MIN_W).floor() as usize).max(1)
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    if st.store.dock.apps.is_empty() {
        draw_empty(ui, st, area);
        return;
    }
    let cols = columns(area.w);
    let tile_w = area.w / cols as f32;
    let tile = |i: usize| Rect::new(area.x + (i % cols) as f32 * tile_w, area.y + (i / cols) as f32 * (TILE_H + ROW_GAP), tile_w, TILE_H);
    let card = super::clipboard::pill_card(st, area);
    let apps = st.store.dock.apps.clone();
    let mut launch = None;
    let mut remove = None;
    for (i, app) in apps.iter().enumerate() {
        let r = tile(i);
        if r.bottom() > area.bottom() + 0.5 {
            break;
        }
        match app_tile(ui, st, r, i, app, card) {
            Some(TileAction::Launch) => launch = Some(i),
            Some(TileAction::Remove) => remove = Some(i),
            None => {}
        }
    }
    let add = tile(apps.len());
    if !st.store.dock.is_full() && add.bottom() <= area.bottom() + 0.5 && add_tile(ui, st, add, card) {
        st.cmd(Command::PickDockApp);
    }
    if let Some(i) = remove {
        if let Some(app) = st.store.dock.remove(i) {
            crate::pinfo!("dock: {} taken off", app.name);
            st.cmd(Command::SaveStore);
        }
    } else if let Some(i) = launch {
        st.cmd(Command::LaunchApp(apps[i].path.clone()));
        st.cmd(Command::Collapse);
    }
}

enum TileAction {
    Launch,
    Remove,
}

fn app_tile(ui: &mut Ui, st: &mut AppState, r: Rect, i: usize, app: &DockApp, card: Option<(Rect, f32)>) -> Option<TileAction> {
    let box_r = r.inset(3.0, 0.0);
    let resp = ui.button(wid_n("dock.app", i as u64), box_r);
    if resp.hovered || resp.pressed {
        super::clipboard::fill_box(ui, card, box_r, TILE_R, Color::white(if resp.pressed { 0.12 } else { 0.07 }));
    }
    let icon_r = Rect::from_center(Point::new(r.center().x, r.y + ICON_TOP + ICON / 2.0), ICON, ICON);
    match icon_of(st, &app.path) {
        // The icon fills ICON_FILL of the picture: draw the picture that much larger.
        Some(img) => ui.c.image(&img, Rect::from_center(icon_r.center(), ICON / ICON_FILL, ICON / ICON_FILL), 0.0),
        None => ui.c.icon(Icon::App, icon_r.inset(4.0, 4.0), palette::TEXT_DIM),
    }
    let name = Rect::new(box_r.x + 4.0, r.y + NAME_TOP, box_r.w - 8.0, NAME_H);
    ui.c.text(&app.name, name, &TextStyle::caption().color(if resp.hovered { palette::TEXT } else { palette::TEXT_DIM }).center());

    // Registered after the tile, so it wins the hit test where they overlap.
    if resp.hovered || ui.hovered(remove_rect(box_r)) {
        let rr = remove_rect(box_r);
        let x = ui.button(wid_n("dock.remove", i as u64), rr);
        let fill = if x.pressed { palette::SURFACE_PRESSED } else if x.hovered { Color::white(0.24) } else { palette::SURFACE_HOVER };
        ui.c.fill_circle(rr.center(), REMOVE_D / 2.0, fill);
        ui.c.icon(Icon::Close, Rect::from_center(rr.center(), 10.0, 10.0), if x.hovered { palette::TEXT } else { palette::TEXT_DIM });
        if x.clicked {
            return Some(TileAction::Remove);
        }
    }
    resp.clicked.then_some(TileAction::Launch)
}

fn remove_rect(tile: Rect) -> Rect {
    Rect::new(tile.right() - REMOVE_D - 3.0, tile.y + 3.0, REMOVE_D, REMOVE_D)
}

fn add_tile(ui: &mut Ui, st: &AppState, r: Rect, card: Option<(Rect, f32)>) -> bool {
    let box_r = r.inset(3.0, 0.0);
    let resp = ui.button(wid("dock.add"), box_r);
    if resp.hovered || resp.pressed {
        super::clipboard::fill_box(ui, card, box_r, TILE_R, Color::white(if resp.pressed { 0.12 } else { 0.07 }));
    }
    let c = Point::new(r.center().x, r.y + ICON_TOP + ICON / 2.0);
    ui.c.fill_circle(c, ICON / 2.0, st.accent.with_alpha(if resp.hovered { 0.28 } else { 0.18 }));
    ui.c.icon(Icon::Plus, Rect::from_center(c, 18.0, 18.0), st.accent);
    let name = Rect::new(box_r.x + 4.0, r.y + NAME_TOP, box_r.w - 8.0, NAME_H);
    ui.c.text("Add", name, &TextStyle::caption().color(if resp.hovered { palette::TEXT } else { palette::TEXT_DIM }).center());
    resp.clicked
}

fn draw_empty(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let top = Rect::new(area.x, area.y, area.w, area.h - 44.0);
    super::empty_state(ui, top, Icon::Dock, "Your apps, one click away", "Add the programs and shortcuts you open most.");
    let label = "Add app";
    let w = ui.pill_width(label, true, false);
    let r = Rect::new(area.center().x - w / 2.0, top.bottom() + 2.0, w, PILL_REGULAR);
    if ui.pill(wid("dock.add.empty"), r, label, Some(Icon::Plus), PillStyle::Primary(st.accent)).clicked {
        st.cmd(Command::PickDockApp);
    }
}

/// The app's icon, asked of Windows the first time (None while it comes,
/// or when Windows has none).
fn icon_of(st: &mut AppState, path: &str) -> Option<Image> {
    match st.dock_icons.get(path) {
        Some(found) => found.clone(),
        None => {
            st.cmd(Command::LoadDockIcon(path.to_string()));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rows_hold_every_app_and_the_add_tile_at_the_narrowest_island() {
        // The narrowest island (480) less the page padding.
        let w = 480.0 - 2.0 * super::super::PAD;
        assert!(columns(w) * 2 > dyna_core::dock::MAX_APPS, "{} columns", columns(w));
        assert!(w / columns(w) as f32 >= TILE_MIN_W);
    }
}
