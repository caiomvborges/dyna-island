//! Where Dyna Island keeps its files.
//!
//! - Settings + data: `%APPDATA%\DynaIsland\` (roams with the user profile)
//! - Cache + log:     `%LOCALAPPDATA%\DynaIsland\`
//!
//! `DYNA_HOME` overrides both (used by `--selftest` to stay isolated).
//!
//! Versions before the rename kept them in `Poni` folders; [`migrate_from_poni`]
//! brings them over once.

use std::path::PathBuf;

/// The folder's name under `%APPDATA%` and `%LOCALAPPDATA%`, and the one
/// versions before the rename used.
const DIR: &str = "DynaIsland";
const OLD_DIR: &str = "Poni";

fn home_override() -> Option<PathBuf> {
    std::env::var_os("DYNA_HOME").map(PathBuf::from)
}

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn data_dir() -> PathBuf {
    home_override().unwrap_or_else(|| env_dir("APPDATA").join(DIR))
}

pub fn cache_dir() -> PathBuf {
    home_override().map(|h| h.join("cache")).unwrap_or_else(|| env_dir("LOCALAPPDATA").join(DIR))
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn store_path() -> PathBuf {
    data_dir().join("data.json")
}

pub fn log_path() -> PathBuf {
    cache_dir().join("dyna-island.log")
}

/// Bring the settings, data and cache over from the `Poni` folders of the
/// versions before the rename, the first time this one starts: the folder
/// is moved whole, or copied when Windows won't move it (a file still open).
/// Nothing happens once the new folder exists, or with `DYNA_HOME` set.
/// Returns what it did, for the log (which lives in one of these folders, so
/// it can only be written afterwards).
pub fn migrate_from_poni() -> Vec<String> {
    if home_override().is_some() {
        return Vec::new();
    }
    let mut done = Vec::new();
    for var in ["APPDATA", "LOCALAPPDATA"] {
        let base = env_dir(var);
        let (old, new) = (base.join(OLD_DIR), base.join(DIR));
        if !old.is_dir() || new.exists() {
            continue;
        }
        let how = match std::fs::rename(&old, &new) {
            Ok(()) => "moved".to_string(),
            Err(e) => match copy_dir(&old, &new) {
                Ok(n) => format!("copied ({n} files; move failed: {e})"),
                Err(c) => format!("NOT brought over (move: {e}; copy: {c})"),
            },
        };
        done.push(format!("{var}\\{OLD_DIR} {how} to {}", new.display()));
    }
    done
}

/// Copy the folder `from` to `to` with everything in it; the number of files.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<usize> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            n += copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_dir_copies_everything_inside() {
        let root = std::env::temp_dir().join(format!("dyna-copy-test-{}", std::process::id()));
        let (from, to) = (root.join("from"), root.join("to"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(from.join("sub")).unwrap();
        std::fs::write(from.join("settings.json"), "{}").unwrap();
        std::fs::write(from.join("sub").join("a.txt"), "a").unwrap();
        assert_eq!(copy_dir(&from, &to).unwrap(), 2);
        assert_eq!(std::fs::read_to_string(to.join("settings.json")).unwrap(), "{}");
        assert_eq!(std::fs::read_to_string(to.join("sub").join("a.txt")).unwrap(), "a");
        let _ = std::fs::remove_dir_all(&root);
    }
}
