//! User settings, stored as JSON next to the data store.

use crate::pomodoro::PomodoroConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleStyle {
    /// Small pill with a clock and battery ring.
    Clock,
    /// Small blank pill (like a MacBook notch).
    Minimal,
    /// A friendly pair of blinking eyes.
    Eyes,
    /// Nothing visible until the cursor reaches the top edge.
    Hidden,
    /// A wider pill like a status bar: the date and time, the volume and
    /// the battery (the Pill theme; the other themes show the clock).
    Status,
}

/// How the island looks. Pages and live activities work the same in every theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// The plain black island.
    #[default]
    Classic,
    /// A light island: warm cream ground, dark ink and an amber accent.
    /// Settings files from before the name change say "oyen".
    #[serde(alias = "oyen")]
    Light,
    /// Liquid glass: the island bends, frosts and tints what's under it.
    Glass,
    /// Pill: the island floats a little below the top edge as a black
    /// capsule, with its own layouts: the time and battery at rest, the song
    /// while music plays, a rounded card when it opens.
    Pill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorChoice {
    Primary,
    /// The monitor under the cursor when Poni starts.
    Cursor,
}

/// Bumped when defaults change in a way existing files should pick up.
pub const SETTINGS_VERSION: u32 = 3;

fn version_zero() -> u32 {
    0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Missing in files written before versioning (treated as 0).
    #[serde(default = "version_zero")]
    pub settings_version: u32,
    pub theme: Theme,
    pub idle_style: IdleStyle,
    pub use_24h: bool,
    /// Delay before hover expands the island.
    pub hover_delay_ms: u32,
    /// Grace period after the cursor leaves before collapsing (0 = the
    /// island starts closing on the next frame).
    pub collapse_delay_ms: u32,
    /// Width of the expanded island in DIPs (bigger = roomier pages).
    pub expanded_width: f32,
    pub hide_in_fullscreen: bool,
    /// Over maximized windows: Step aside (true) or Stay. While an app window
    /// reaches the top of the screen under the island (a maximized browser's
    /// tabs, a snapped window's title bar), the resting island turns into the
    /// Hidden look's invisible strip until the pointer reaches it; volume,
    /// notices and a new song's title still show. Windows only: on a Mac the
    /// menu bar keeps windows below the island.
    pub step_aside: bool,
    pub monitor: MonitorChoice,
    pub show_media_activity: bool,
    pub hud_volume: bool,
    pub hud_power: bool,
    pub hud_bluetooth: bool,
    pub clipboard_history: bool,
    pub sounds: bool,
    /// Page shown first when expanding ("media", "focus", "tasks", "clipboard", "today", …).
    pub start_page: String,
    /// Remember the last page instead of `start_page`.
    pub remember_page: bool,
    pub pomodoro: PomodoroConfig,
    /// Keep the System page's charts going while it's closed (a light reading
    /// of the kernel counters once a second), so they're full when it opens.
    pub system_history: bool,
    /// The Outlook agenda add-on's calendar: the ICS link Outlook publishes
    /// (`ics::check_link` holds it to Outlook's own hosts). Empty: none yet.
    pub agenda_link: String,
    /// Add-ons switched on, by id (`poni_core::addons`). Ids this version
    /// doesn't know are kept, so a newer Poni's choices survive a save.
    pub addons: BTreeMap<String, bool>,
    /// Show over games: while a full-screen app is in front, the island
    /// shows its frame rate, the GPU's load and temperature and the CPU's
    /// load. Turning it on also keeps the island on top of full-screen apps.
    pub game_stats: bool,
    /// Show over games, readout only for a few seconds (when the game comes
    /// to the front, when the frame rate drops sharply, on Alt+Shift+F),
    /// then the island hides and Windows sends the game's frames straight to
    /// the screen. Off (the default), the readout stays while the game is in
    /// front. Its own name: the corner test build wrote `game_readout`
    /// ("peek" as its default) into the files of the people who tried it.
    pub game_peek: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_version: SETTINGS_VERSION,
            theme: Theme::Classic,
            idle_style: IdleStyle::Clock,
            use_24h: true,
            hover_delay_ms: 220,
            collapse_delay_ms: 0,
            expanded_width: 620.0,
            hide_in_fullscreen: true,
            step_aside: true,
            monitor: MonitorChoice::Primary,
            show_media_activity: true,
            hud_volume: true,
            hud_power: true,
            hud_bluetooth: true,
            clipboard_history: true,
            sounds: true,
            start_page: "media".into(),
            remember_page: true,
            pomodoro: PomodoroConfig::default(),
            system_history: true,
            agenda_link: String::new(),
            addons: BTreeMap::new(),
            game_stats: false,
            game_peek: false,
        }
    }
}

impl Settings {
    /// Show over games needs the island on top of full-screen apps, so the
    /// two follow each other: turning the readout on picks Stay on top, and
    /// picking Hide afterwards turns the readout off. `before` is the state
    /// the change started from.
    pub fn pair_game_stats(&mut self, before: &Settings) {
        if !(self.game_stats && self.hide_in_fullscreen) {
            return;
        }
        if before.game_stats && !before.hide_in_fullscreen {
            self.game_stats = false;
        } else {
            self.hide_in_fullscreen = false;
        }
    }

    /// Is the add-on `id` switched on?
    pub fn addon_on(&self, id: &str) -> bool {
        self.addons.get(id).copied().unwrap_or(false)
    }

    pub fn set_addon(&mut self, id: &str, on: bool) {
        self.addons.insert(id.to_string(), on);
    }

    /// Has the person never switched the add-on `id` on or off? An add-on
    /// that switches itself on when first needed (Japanese readings) does so
    /// only then: one switched off by hand stays off.
    pub fn addon_unset(&self, id: &str) -> bool {
        !self.addons.contains_key(id)
    }

    /// Clamp values a user may have hand-edited into nonsense.
    pub fn sanitized(mut self) -> Self {
        // Only untouched old defaults move.
        // v2: hover opens after a 220 ms dwell (140 ms caught pointers on
        // their way to browser tabs).
        if self.settings_version < 2 && self.hover_delay_ms == 140 {
            self.hover_delay_ms = 220;
        }
        // v3: the island closes as soon as the pointer leaves it (the old
        // 380 ms grace read as lag; coming back while it closes reopens it).
        if self.settings_version < 3 && self.collapse_delay_ms == 380 {
            self.collapse_delay_ms = 0;
        }
        self.settings_version = self.settings_version.max(SETTINGS_VERSION);
        // Show over games needs the island on top of full-screen apps.
        if self.game_stats {
            self.hide_in_fullscreen = false;
        }
        self.hover_delay_ms = self.hover_delay_ms.min(2000);
        self.collapse_delay_ms = self.collapse_delay_ms.min(5000);
        if !self.expanded_width.is_finite() {
            self.expanded_width = 620.0;
        }
        self.expanded_width = self.expanded_width.clamp(480.0, 900.0);
        let p = &mut self.pomodoro;
        p.focus_min = p.focus_min.clamp(1, 180);
        p.short_break_min = p.short_break_min.clamp(1, 60);
        p.long_break_min = p.long_break_min.clamp(1, 120);
        p.long_every = p.long_every.min(12);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_fills_defaults() {
        // An old file's "water" block (the page Health replaced) is ignored.
        let s: Settings = serde_json::from_str(r#"{"idle_style":"eyes","water":{"goal_ml":1500}}"#).unwrap();
        assert_eq!(s.idle_style, IdleStyle::Eyes);
        assert_eq!(s.pomodoro.focus_min, 25);
    }

    #[test]
    fn addons_are_off_until_switched_on_and_unknown_ones_survive() {
        let mut s = Settings::default();
        assert!(!s.addon_on("addon"));
        s.set_addon("addon", true);
        assert!(s.addon_on("addon"));
        // A newer Poni's add-on stays in the file when this one saves it.
        let s: Settings = serde_json::from_str(r#"{"addons":{"addon":true,"future":true}}"#).unwrap();
        assert!(s.addon_on("addon"));
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(back.addon_on("future"));
        // Files from before add-ons have none on.
        let old: Settings = serde_json::from_str(r#"{"settings_version":3}"#).unwrap();
        assert!(old.addons.is_empty());
    }

    #[test]
    fn an_addon_switched_off_by_hand_is_no_longer_unset() {
        let mut s = Settings::default();
        assert!(s.addon_unset("ja"));
        s.set_addon("ja", true);
        assert!(!s.addon_unset("ja"));
        s.set_addon("ja", false);
        assert!(!s.addon_unset("ja"), "off by hand: the person's word, kept");
        assert!(!s.addon_on("ja"));
        // It survives a save and a load.
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert!(!back.addon_unset("ja"));
    }

    #[test]
    fn theme_defaults_to_classic() {
        // Files written before themes existed keep the black island.
        let s: Settings = serde_json::from_str(r#"{"idle_style":"eyes"}"#).unwrap();
        assert_eq!(s.theme, Theme::Classic);
        let s: Settings = serde_json::from_str(r#"{"theme":"light"}"#).unwrap();
        assert_eq!(s.theme, Theme::Light);
        let s: Settings = serde_json::from_str(r#"{"theme":"oyen"}"#).unwrap();
        assert_eq!(s.theme, Theme::Light, "the old name reads as Light");
        let s: Settings = serde_json::from_str(r#"{"theme":"glass"}"#).unwrap();
        assert_eq!(s.theme, Theme::Glass);
        let s: Settings = serde_json::from_str(r#"{"theme":"pill"}"#).unwrap();
        assert_eq!(s.theme, Theme::Pill);
    }

    #[test]
    fn sanitize_clamps() {
        let s = Settings { expanded_width: 5000.0, hover_delay_ms: 99999, ..Default::default() }.sanitized();
        assert_eq!(s.expanded_width, 900.0);
        assert_eq!(s.hover_delay_ms, 2000);
    }

    #[test]
    fn migrates_old_hover_default_once() {
        let s: Settings = serde_json::from_str(r#"{"hover_delay_ms":140}"#).unwrap();
        assert_eq!(s.settings_version, 0);
        let s = s.sanitized();
        assert_eq!((s.hover_delay_ms, s.settings_version), (220, SETTINGS_VERSION));
        // A user's own choice is kept, and a current file isn't touched.
        let s: Settings = serde_json::from_str(r#"{"hover_delay_ms":90}"#).unwrap();
        assert_eq!(s.sanitized().hover_delay_ms, 90);
        let s: Settings = serde_json::from_str(r#"{"settings_version":2,"hover_delay_ms":140}"#).unwrap();
        assert_eq!(s.sanitized().hover_delay_ms, 140);
    }

    #[test]
    fn migrates_old_collapse_grace_once() {
        // Files from 0.1.7 and earlier hold the old 380 ms grace.
        let s = serde_json::from_str::<Settings>(r#"{"settings_version":2,"collapse_delay_ms":380}"#).unwrap().sanitized();
        assert_eq!((s.collapse_delay_ms, s.settings_version), (0, SETTINGS_VERSION));
        let s = serde_json::from_str::<Settings>(r#"{"collapse_delay_ms":380,"hover_delay_ms":140}"#).unwrap().sanitized();
        assert_eq!((s.collapse_delay_ms, s.hover_delay_ms), (0, 220));
        // A grace the user chose is kept, a current file isn't touched, and 0 stays 0.
        let s: Settings = serde_json::from_str(r#"{"settings_version":2,"collapse_delay_ms":600}"#).unwrap();
        assert_eq!(s.sanitized().collapse_delay_ms, 600);
        let s: Settings = serde_json::from_str(r#"{"settings_version":3,"collapse_delay_ms":380}"#).unwrap();
        assert_eq!(s.sanitized().collapse_delay_ms, 380);
        assert_eq!(Settings::default().sanitized().collapse_delay_ms, 0);
        // A file from a newer Poni keeps its version.
        let s: Settings = serde_json::from_str(r#"{"settings_version":9}"#).unwrap();
        assert_eq!(s.sanitized().settings_version, 9);
    }

    #[test]
    fn game_stats_and_full_screen_choice_follow_each_other() {
        // The default hides the island over full-screen apps; turning the
        // readout on keeps it on top instead.
        let before = Settings::default();
        let mut s = Settings { game_stats: true, ..before.clone() };
        s.pair_game_stats(&before);
        assert!(s.game_stats && !s.hide_in_fullscreen);
        // Picking Hide afterwards turns the readout off.
        let before = s.clone();
        let mut s = Settings { hide_in_fullscreen: true, ..before.clone() };
        s.pair_game_stats(&before);
        assert!(!s.game_stats && s.hide_in_fullscreen);
        // Stay on top alone, and the readout off, change nothing.
        let before = Settings { hide_in_fullscreen: false, ..Settings::default() };
        let mut s = before.clone();
        s.pair_game_stats(&before);
        assert_eq!(s, before);
        // A hand-edited file with both on keeps the readout, when changed
        // and when loaded.
        let mut s = Settings { game_stats: true, ..Settings::default() };
        s.pair_game_stats(&Settings::default());
        assert!(s.game_stats && !s.hide_in_fullscreen);
        let s: Settings = serde_json::from_str(r#"{"game_stats":true,"hide_in_fullscreen":true}"#).unwrap();
        let s = s.sanitized();
        assert!(s.game_stats && !s.hide_in_fullscreen);
    }

    #[test]
    fn the_readout_stays_unless_peeks_are_picked() {
        // Files from 0.2.0 and before, and from the corner test build (its
        // "peek" was its default, not a choice), get the readout that stays.
        for old in [r#"{"game_stats":true}"#, r#"{"game_stats":true,"game_readout":"peek"}"#, r#"{"game_readout":"corner"}"#] {
            let s: Settings = serde_json::from_str(old).unwrap();
            assert!(!s.game_peek, "{old}");
        }
        let s: Settings = serde_json::from_str(r#"{"game_stats":true,"game_peek":true}"#).unwrap();
        assert!(s.game_peek);
    }

    #[test]
    fn the_island_steps_aside_unless_stay_is_picked() {
        // Files from 0.2.2 and before have no choice yet: Step aside.
        let old: Settings = serde_json::from_str(r#"{"settings_version":3,"hide_in_fullscreen":false}"#).unwrap();
        assert!(old.step_aside);
        let stay: Settings = serde_json::from_str(r#"{"step_aside":false}"#).unwrap();
        assert!(!stay.step_aside);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&stay).unwrap()).unwrap();
        assert!(!back.step_aside);
    }

    #[test]
    fn roundtrip() {
        let s = Settings::default();
        let j = serde_json::to_string_pretty(&s).unwrap();
        let back: Settings = serde_json::from_str(&j).unwrap();
        assert_eq!(s, back);
    }
}
