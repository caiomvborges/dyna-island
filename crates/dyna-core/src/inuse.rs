//! The camera and microphone in use, as Windows' privacy records keep them
//! (`CapabilityAccessManager\ConsentStore`): which apps hold each right now,
//! by a name people know them by.

/// The apps using the camera and the microphone right now, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InUse {
    pub camera: Vec<String>,
    pub mic: Vec<String>,
}

impl InUse {
    pub fn any(&self) -> bool {
        !self.camera.is_empty() || !self.mic.is_empty()
    }

    /// What the pop-up says.
    pub fn label(&self) -> &'static str {
        match (!self.camera.is_empty(), !self.mic.is_empty()) {
            (true, true) => "Camera and mic in use",
            (true, false) => "Camera in use",
            (false, true) => "Microphone in use",
            (false, false) => "",
        }
    }

    /// The apps using either, each once, the camera's first: "Teams, Edge".
    pub fn apps(&self) -> String {
        let mut out: Vec<&str> = Vec::new();
        for a in self.camera.iter().chain(&self.mic) {
            if !out.contains(&a.as_str()) {
                out.push(a);
            }
        }
        out.join(", ")
    }

    /// An app started using the camera or the microphone since `old`.
    pub fn started_since(&self, old: &InUse) -> bool {
        self.camera.iter().any(|a| !old.camera.contains(a)) || self.mic.iter().any(|a| !old.mic.contains(a))
    }

    /// Names in a steady order, each once (two copies of an app, or one
    /// recorded for both the user and the machine, count once).
    pub fn tidy(mut self) -> Self {
        for list in [&mut self.camera, &mut self.mic] {
            list.sort();
            list.dedup();
        }
        self
    }
}

/// The name of the app a privacy record's key stands for. Desktop apps are
/// their path with `#` for `\` ("C:#Program Files#…#msedge.exe"), Store apps
/// their package family ("MSTeams_8wekyb3d8bbwe").
pub fn app_name(key: &str) -> String {
    let base = if key.contains('#') {
        let file = key.rsplit('#').next().unwrap_or(key);
        match file.len().checked_sub(4) {
            Some(n) if file.is_char_boundary(n) && file[n..].eq_ignore_ascii_case(".exe") => &file[..n],
            _ => file,
        }
    } else {
        let family = key.split('_').next().unwrap_or(key);
        family.rsplit('.').next().unwrap_or(family)
    };
    let known = [
        ("msedge", "Edge"),
        ("msedgewebview2", "Edge"),
        ("chrome", "Chrome"),
        ("firefox", "Firefox"),
        ("brave", "Brave"),
        ("opera", "Opera"),
        ("msteams", "Teams"),
        ("ms-teams", "Teams"),
        ("teams", "Teams"),
        ("zoom", "Zoom"),
        ("discord", "Discord"),
        ("slack", "Slack"),
        ("skype", "Skype"),
        ("whatsapp", "WhatsApp"),
        ("whatsappdesktop", "WhatsApp"),
        ("obs64", "OBS"),
        ("obs32", "OBS"),
        ("obs", "OBS"),
        ("windowscamera", "Camera"),
        ("windowssoundrecorder", "Sound Recorder"),
        ("outlookforwindows", "Outlook"),
        ("lenovocompanion", "Lenovo Vantage"),
    ];
    match known.iter().find(|(k, _)| k.eq_ignore_ascii_case(base)) {
        Some((_, name)) => name.to_string(),
        None => {
            let mut c = base.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_people_know() {
        assert_eq!(app_name("C:#Program Files (x86)#Microsoft#Edge#Application#msedge.exe"), "Edge");
        assert_eq!(app_name("C:#Users#me#AppData#Local#Discord#app-1.0.9249#Discord.exe"), "Discord");
        assert_eq!(app_name("MSTeams_8wekyb3d8bbwe"), "Teams");
        assert_eq!(app_name("Microsoft.WindowsCamera_8wekyb3d8bbwe"), "Camera");
        assert_eq!(app_name("E046963F.LenovoCompanion_k1h2ywk1493x8"), "Lenovo Vantage");
        assert_eq!(app_name("C:#Tools#recorder.EXE"), "Recorder");
        assert_eq!(app_name("C:#Tools#ünicode"), "Ünicode");
    }

    #[test]
    fn what_the_pop_up_says() {
        let none = InUse::default();
        let call = InUse { camera: vec!["Teams".into()], mic: vec!["Edge".into(), "Teams".into()] };
        assert!(!none.any() && call.any());
        assert_eq!(call.label(), "Camera and mic in use");
        assert_eq!(call.apps(), "Teams, Edge");
        assert!(call.started_since(&none));
        assert!(!call.started_since(&call));
        // Stopping isn't starting.
        assert!(!none.started_since(&call));
        let mic_only = InUse { camera: vec![], mic: vec!["Zoom".into(), "Zoom".into()] }.tidy();
        assert_eq!((mic_only.label(), mic_only.mic.len()), ("Microphone in use", 1));
    }
}
