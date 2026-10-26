//! Add-ons: features that stay out of the way until the person switches them
//! on. Each one's code is part of the app and stays asleep while it is off;
//! nothing is ever downloaded.

/// A built-in add-on, as the Add-ons page names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Addon {
    /// Stable id: the key in settings.json.
    pub id: &'static str,
    pub title: &'static str,
    /// What it does, in one line, for the Add-ons page.
    pub blurb: &'static str,
}

impl Addon {
    pub fn find(id: &str) -> Option<&'static Addon> {
        ALL.iter().copied().find(|a| a.id == id)
    }
}

/// Script reader: the Script page, a teleprompter under the webcam. Off, the
/// island has no Script tab, no Alt+Shift+S and nothing running for it.
pub const SCRIPT_READER: Addon = Addon {
    id: "script",
    title: "Script reader",
    blurb: "A teleprompter under your webcam, left out of screen shares.",
};

/// Outlook agenda: the next meeting counting down on the island and an Agenda
/// tab, read from the calendar link Outlook publishes (the app's only network
/// connection, to that link only). Off, nothing is read.
pub const OUTLOOK_AGENDA: Addon = Addon {
    id: "outlook",
    title: "Outlook agenda",
    blurb: "Your next meeting on the island, from your Outlook calendar's link.",
};

/// Every add-on, in the order the Add-ons page lists them.
pub const ALL: [&Addon; 2] = [&SCRIPT_READER, &OUTLOOK_AGENDA];

/// Where an add-on stands, as the Add-ons page shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Switched off: nothing running.
    Off,
    /// Switched on.
    Ready,
}

impl Status {
    /// The switch shows on for everything but `Off`.
    pub fn is_on(self) -> bool {
        self != Status::Off
    }

    /// The line under the add-on's name.
    pub fn line(self, addon: &Addon) -> String {
        match self {
            Status::Off => addon.blurb.to_string(),
            Status::Ready => "On".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_reader_is_listed_and_says_what_it_does() {
        assert_eq!(Addon::find("script"), Some(&SCRIPT_READER));
        assert!(Addon::find("nope").is_none());
        assert_eq!(Status::Ready.line(&SCRIPT_READER), "On");
        assert_eq!(Status::Off.line(&SCRIPT_READER), SCRIPT_READER.blurb);
        assert!(!Status::Off.is_on() && Status::Ready.is_on());
    }
}
