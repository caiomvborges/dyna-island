//! Which build this is, for the menu's About line and the tray tooltip:
//! the version, plus where the build came from (build.rs fills these in).

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// "release" (a release tag's CI build), "test" (any other CI build) or "local".
const KIND: &str = env!("PONI_BUILD_KIND");
/// The commit a CI build was made from, 7 characters; empty for a local build.
const COMMIT: &str = env!("PONI_BUILD_COMMIT");
/// The UTC day a CI build was made, "25 Oct 2026"; empty for a local build.
const DAY: &str = env!("PONI_BUILD_DAY");

/// "About Dyna Island 0.2.3 (25 Oct 2026)"; a test build adds its commit.
pub fn about_label() -> String {
    label(VERSION, KIND, COMMIT, DAY)
}

fn label(version: &str, kind: &str, commit: &str, day: &str) -> String {
    let detail = match kind {
        "release" => day.to_string(),
        "test" if day.is_empty() => format!("test build {commit}"),
        "test" => format!("test build {commit}, {day}"),
        _ => "local build".to_string(),
    };
    if detail.is_empty() { format!("About Dyna Island {version}") } else { format!("About Dyna Island {version} ({detail})") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_about_line_says_which_build_this_is() {
        assert_eq!(label("0.2.3", "release", "c7d77c8", "25 Oct 2026"), "About Dyna Island 0.2.3 (25 Oct 2026)");
        assert_eq!(label("0.2.3", "test", "c7d77c8", "25 Oct 2026"), "About Dyna Island 0.2.3 (test build c7d77c8, 25 Oct 2026)");
        assert_eq!(label("0.2.3", "local", "", ""), "About Dyna Island 0.2.3 (local build)");
        assert!(about_label().starts_with(&format!("About Dyna Island {VERSION}")));
    }
}
