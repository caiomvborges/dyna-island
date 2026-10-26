//! All state shared by pages, activities and the app loop.
//!
//! Owned by the UI thread. Services never touch it directly; they send
//! [`crate::events::AppEvent`]s that the app applies here.

use crate::commands::{Command, MediaCmd, Sound};
use crate::gfx::Image;
use crate::icons::Icon;
use crate::pages::PageKind;
use poni_core::addons::Status as AddonStatus;
use poni_core::cliphist::ClipHistory;
use poni_core::datetime::LocalDateTime;
use poni_core::geom::{palette, Color};
use poni_core::pomodoro::Pomodoro;
use poni_core::settings::Settings;
use poni_core::store::Store;
use poni_core::timeline::Timeline;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
pub struct MediaSession {
    /// AppUserModelId of the player (e.g. `Spotify.exe`, `Chrome`).
    pub app_id: String,
    /// Friendly player name ("Spotify", "Chrome", …).
    pub app_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: Option<Image>,
    /// Accent derived from the art (for progress bars / the activity glow).
    pub art_color: Option<Color>,
    pub timeline: Timeline,
    pub can_prev: bool,
    pub can_next: bool,
    pub can_play_pause: bool,
    pub can_seek: bool,
    /// Identity of the track: `title\u{1}artist\u{1}album`.
    pub track_key: String,
}

impl MediaSession {
    pub fn make_track_key(title: &str, artist: &str, album: &str) -> String {
        format!("{title}\u{1}{artist}\u{1}{album}")
    }
    pub fn is_playing(&self) -> bool {
        self.timeline.playing
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VolumeState {
    pub level: f32,
    pub muted: bool,
    pub known: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerState {
    pub has_battery: bool,
    /// 0..=100 (meaningful only with a battery).
    pub percent: u8,
    pub charging: bool,
    pub on_ac: bool,
    pub saver: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SysStats {
    pub valid: bool,
    /// 0..=1
    pub cpu: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub disk_free: u64,
    pub disk_total: u64,
    pub uptime_s: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HudKind {
    Volume { level: f32, muted: bool },
    Charging { percent: u8, plugged: bool },
    Bluetooth { name: String, connected: bool, battery: Option<u8>, audio: bool },
    LowBattery { percent: u8 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hud {
    pub kind: HudKind,
    /// Monotonic time when it disappears.
    pub until: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NoticeAction {
    OpenPage(PageKind),
    /// Open Focus and start the (idle) timer.
    StartFocus,
}

/// A one-click action shown as a small pill inside a notice (doesn't expand).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineAction {
    /// Start the idle Pomodoro phase.
    StartFocus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub icon: Icon,
    pub color: Color,
    pub title: String,
    pub subtitle: String,
    pub until: f64,
    pub action: Option<NoticeAction>,
    pub inline: Option<InlineAction>,
}

/// Transient per-page UI state (scroll offsets, text being typed, …).
#[derive(Clone, Debug, Default)]
pub struct PageUiState {
    pub task_input: String,
    pub task_scroll: f32,
    /// The Notes page: what's being typed, the list's scroll, the note just
    /// copied (id, when) and the one just deleted (where it was, it, when).
    pub note_input: String,
    pub note_scroll: f32,
    pub note_flash: Option<(u64, f64)>,
    pub note_undo: Option<(usize, poni_core::notes::Note, f64)>,
    pub clip_scroll: f32,
    /// Clipboard row that just got copied (index, mono time) for a "Copied" flash.
    pub clip_flash: Option<(usize, f64)>,
    /// Volume HUD bar position, gliding toward the real level.
    pub hud_level: f32,
    /// Collapsed media pill shows the track title until this mono time.
    pub peek_until: f64,
    /// Header tab under the pointer and since when (for hover labels).
    pub tab_hover: Option<(usize, f64)>,
    /// The module the System page shows under its strip.
    pub system_module: poni_core::sysmon::Module,
    /// The last System reading had a GPU (keeps its chip while the next one is on its way).
    pub system_gpu: bool,
    /// The last full System reading had the whole PC's power (keeps its chip
    /// while the page's first, light reading shows).
    pub system_power: bool,
    /// The page the Settings button was pressed on (pressing it again goes back).
    pub before_settings: Option<PageKind>,
    /// Start with Windows, as last read off the UI thread (None until Settings first opens).
    pub autostart: Option<bool>,
}

/// Whether Windows lets Dyna Island read a game's frames. Its event tracing
/// is open to admins and to the Performance Log Users group; the one-time
/// OK adds the account to that group, which counts from the next sign-in.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum FpsAccess {
    /// Not asked yet.
    #[default]
    Unknown,
    /// Being checked.
    Checking,
    /// Frames can be read.
    Ready,
    /// The one-time OK is needed.
    NeedsOk,
    /// Windows' prompt for it is open.
    Asking,
    /// The account is in the group; Windows applies it at the next sign-in.
    SignOut,
    /// The OK didn't go through (why, in a few words).
    Failed(String),
}

/// Show over games: the readout in the island over a full-screen app
/// (`activities::Activity::Game`) and the System page's Games view.
#[derive(Clone, Debug, Default)]
pub struct GameState {
    /// The readout is on screen: Show over games is on and a full-screen
    /// app is in front.
    pub active: bool,
    /// The full-screen app ("cs2.exe").
    pub app: Option<String>,
    /// Its frame rate, while it draws and the frames can be read.
    pub rate: Option<poni_core::frames::FrameRate>,
    /// Frames per second over the last minute in front.
    pub history: poni_core::sysmon::Series,
    pub access: FpsAccess,
}

/// A process watched until it ends (`services::watch`).
#[derive(Clone, Debug, PartialEq)]
pub struct WatchedProcess {
    pub pid: u32,
    pub name: String,
    pub since: f64,
}

/// Where the Outlook agenda add-on's last read stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AgendaStatus {
    /// Not asked yet (the add-on is off, or just came on).
    #[default]
    Idle,
    Reading,
    /// Read: `AgendaState::meetings` is what Outlook has.
    Ready,
    /// The calendar couldn't be reached (offline): the last meetings stay.
    Offline(String),
    /// Outlook answered something that isn't the calendar (why, in a few words).
    Failed(String),
}

/// The Outlook agenda add-on's meetings (`services::agenda`).
#[derive(Clone, Debug, Default)]
pub struct AgendaState {
    pub status: AgendaStatus,
    /// Today's and tomorrow's, in order (`poni_core::agenda::tidy`).
    pub meetings: Vec<poni_core::agenda::Meeting>,
    /// When Outlook last answered, as the island's clock said it ("14:32").
    pub read_at: Option<String>,
    /// Why a pasted link wasn't taken, until mono time `.1`.
    pub note: Option<(&'static str, f64)>,
}

pub struct AppState {
    pub settings: Settings,
    pub store: Store,
    /// Monotonic seconds at the current frame.
    pub now: f64,
    pub local: LocalDateTime,
    pub unix: i64,
    pub media: Option<MediaSession>,
    pub pomodoro: Pomodoro,
    pub volume: VolumeState,
    pub power: PowerState,
    pub hud: Option<Hud>,
    pub notice: Option<Notice>,
    pub clip: ClipHistory,
    pub stats: SysStats,
    /// The System page's readings and their last minute (empty while it's closed).
    pub system: poni_core::sysmon::Monitor,
    /// Show over games.
    pub game: GameState,
    /// Icons of the apps the System page lists, by executable path (None = loading or none).
    pub app_icons: HashMap<String, Option<Image>>,
    /// The Outlook agenda add-on's meetings, while it is on.
    pub agenda: AgendaState,
    /// Processes the person asked to hear about when they end (the System
    /// page's bell), by pid, with their name and when the watch began (mono s).
    pub watched: Vec<WatchedProcess>,
    /// Icons of the Dock's apps, by path (None = loading or none).
    pub dock_icons: HashMap<String, Option<Image>>,
    pub ui: PageUiState,
    /// System accent colour (falls back to blue).
    pub accent: Color,
    /// The page currently shown when expanded.
    pub page: PageKind,
    /// When playback went from playing to paused (the pill lingers 30 s).
    pub media_paused_at: Option<f64>,
    pub pinned: bool,
    /// Stepped aside (Settings › Over maximized windows): an app window sits
    /// under where the island rests, so the closed island rests as an
    /// invisible strip at the top edge (`activities::Mode::Aside`).
    pub aside: bool,
    /// Files are being dragged over the island right now.
    pub drag_hover: bool,
    /// Where each add-on stands, by id (every one in `poni_core::addons::ALL`).
    pub addons: BTreeMap<&'static str, AddonStatus>,
    /// The Script page's reading (its script and pace are in `store.script`).
    pub script: poni_core::script::Reader,
    /// A message under the script until a time (mono seconds).
    pub script_note: Option<(&'static str, f64)>,
    /// Whether Windows leaves the island out of screen capture while the
    /// Script page shows (None = not asked yet, or not on Windows).
    pub script_hidden: Option<bool>,
    /// Side effects requested during the frame.
    pub cmds: Vec<Command>,
}

impl AppState {
    /// A reading under way on the Script page keeps the open island from
    /// closing, while that page is on screen.
    pub fn script_holds_island(&self) -> bool {
        self.page == PageKind::Script && self.script.active
    }

    /// Something on the page on screen keeps the open island from closing.
    pub fn page_holds_island(&self) -> bool {
        self.script_holds_island()
    }

    pub fn new(settings: Settings, store: Store) -> Self {
        let pomodoro = Pomodoro::new(settings.pomodoro.clone());
        let page = PageKind::from_key(if settings.remember_page && !store.last_page.is_empty() {
            &store.last_page
        } else {
            &settings.start_page
        });
        let mut st = Self {
            settings,
            store,
            now: 0.0,
            local: LocalDateTime::default(),
            unix: 0,
            media: None,
            pomodoro,
            volume: VolumeState::default(),
            power: PowerState::default(),
            hud: None,
            notice: None,
            clip: ClipHistory::default(),
            stats: SysStats::default(),
            system: poni_core::sysmon::Monitor::default(),
            game: GameState::default(),
            app_icons: HashMap::new(),
            dock_icons: HashMap::new(),
            watched: Vec::new(),
            agenda: AgendaState::default(),
            ui: PageUiState::default(),
            accent: palette::BLUE,
            page,
            media_paused_at: None,
            pinned: false,
            aside: false,
            drag_hover: false,
            addons: BTreeMap::new(),
            script: poni_core::script::Reader::default(),
            script_note: None,
            script_hidden: None,
            cmds: Vec::new(),
        };
        // On ones start by checking their files (the worker reports back);
        // one without files is ready at once.
        for a in poni_core::addons::ALL {
            let on = st.settings.addon_on(a.id);
            st.addons.insert(a.id, if on { AddonStatus::Ready } else { AddonStatus::Off });
        }
        // Last open on the Script page, since switched off: open where a new
        // island does.
        if (st.page == PageKind::Script && !st.script_on()) || (st.page == PageKind::Agenda && !st.agenda_on()) {
            st.page = PageKind::ALL[0];
        }
        st
    }

    /// Where the add-on `id` stands (`Off` for one this version doesn't know).
    pub fn addon(&self, id: &str) -> AddonStatus {
        self.addons.get(id).copied().unwrap_or(AddonStatus::Off)
    }

    /// The Outlook agenda add-on is on: the Agenda tab and the countdown show.
    pub fn agenda_on(&self) -> bool {
        self.addon(poni_core::addons::OUTLOOK_AGENDA.id) == AddonStatus::Ready
    }

    /// The agenda add-on is on and has a calendar link to read.
    pub fn agenda_reads(&self) -> bool {
        self.agenda_on() && !self.settings.agenda_link.is_empty()
    }

    /// The next meeting while the closed island counts down to it, and
    /// seconds until it starts (`poni_core::agenda::countdown`).
    pub fn meeting_countdown(&self) -> Option<(&poni_core::agenda::Meeting, i64)> {
        if !self.agenda_on() {
            return None;
        }
        poni_core::agenda::countdown(&self.agenda.meetings, poni_core::agenda::local_secs(&self.local))
    }

    /// The Script reader add-on is on: the Script tab shows.
    pub fn script_on(&self) -> bool {
        self.addon(poni_core::addons::SCRIPT_READER.id) == AddonStatus::Ready
    }

    pub fn cmd(&mut self, c: Command) {
        self.cmds.push(c);
    }

    pub fn media_cmd(&mut self, c: MediaCmd) {
        self.cmds.push(Command::Media(c));
    }

    /// Mark the persisted store as changed.
    pub fn save(&mut self) {
        if !self.cmds.contains(&Command::SaveStore) {
            self.cmds.push(Command::SaveStore);
        }
    }

    pub fn chime(&mut self) {
        if self.settings.sounds {
            self.cmds.push(Command::Play(Sound::Chime));
        }
    }

    /// Show a notice pill for `secs` seconds.
    pub fn notify(&mut self, icon: Icon, color: Color, title: impl Into<String>, subtitle: impl Into<String>, secs: f64, action: Option<NoticeAction>) {
        self.notice = Some(Notice { icon, color, title: title.into(), subtitle: subtitle.into(), until: self.now + secs, action, inline: None });
    }

    /// Give the current notice a one-click inline action pill.
    pub fn set_notice_inline(&mut self, a: InlineAction) {
        if let Some(n) = self.notice.as_mut() {
            n.inline = Some(a);
        }
    }

    pub fn show_hud(&mut self, kind: HudKind, secs: f64) {
        self.hud = Some(Hud { kind, until: self.now + secs });
    }

    pub fn day_key(&self) -> String {
        self.local.day_key()
    }

    pub fn day_start_unix(&self) -> i64 {
        crate::clock::day_start_unix(&self.local, self.unix)
    }

    /// Accent for media UI: art colour when available, else the system accent.
    /// The Light theme keeps to its amber.
    pub fn media_accent(&self) -> Color {
        if self.settings.theme == poni_core::settings::Theme::Light {
            return self.accent;
        }
        self.media.as_ref().and_then(|m| m.art_color).unwrap_or(self.accent)
    }
}
