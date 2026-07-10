use std::process::Command;

pub trait PkgUtil {
    /// Package ids that claim ownership of `path`. `None` = pkgutil could not run.
    fn file_info(&self, path: &str) -> Option<Vec<String>>;
    /// Install-relative paths the package wrote. `None` = unknown package or failure.
    fn files(&self, pkg_id: &str) -> Option<Vec<String>>;
    /// Absolute prefix the package's file list is relative to (volume + install
    /// location, e.g. "/" or "/Applications"). `None` = unknown package or failure.
    fn install_root(&self, pkg_id: &str) -> Option<String>;
}

pub struct RealPkgUtil;

impl RealPkgUtil {
    fn run(args: &[&str]) -> Option<String> {
        let out = Command::new("pkgutil").args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl PkgUtil for RealPkgUtil {
    fn file_info(&self, path: &str) -> Option<Vec<String>> {
        // --file-info exits non-zero for unowned paths; treat that as "no owners".
        let out = Command::new("pkgutil")
            .arg("--file-info")
            .arg(path)
            .output()
            .ok()?;
        Some(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| l.strip_prefix("pkgid: "))
                .map(|s| s.trim().to_string())
                .collect(),
        )
    }

    fn files(&self, pkg_id: &str) -> Option<Vec<String>> {
        Some(
            Self::run(&["--files", pkg_id])?
                .lines()
                .map(str::to_string)
                .filter(|l| !l.is_empty())
                .collect(),
        )
    }

    fn install_root(&self, pkg_id: &str) -> Option<String> {
        let info = Self::run(&["--pkg-info", pkg_id])?;
        let field = |key: &str| {
            info.lines()
                .find_map(|l| l.strip_prefix(key))
                .map(|s| s.trim().to_string())
        };
        let volume = field("volume: ")?;
        let location = field("install-location: ").unwrap_or_default();
        Some(join_root(&volume, &location))
    }
}

/// Join a volume ("/", "/Volumes/X") and an install-location ("/", ".", "Applications")
/// into the absolute prefix package file paths are relative to.
pub fn join_root(volume: &str, location: &str) -> String {
    let vol = volume.trim_end_matches('/');
    let loc = location.trim_matches('/').trim_start_matches("./");
    match (vol.is_empty(), loc.is_empty() || loc == ".") {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{loc}"),
        (false, true) => vol.to_string(),
        (false, false) => format!("{vol}/{loc}"),
    }
}

/// The installer package that owns `path` (its first owner), or `None` if the path
/// belongs to no receipt or pkgutil could not be run. Used to show "Installed by"
/// and to group plugins by installer. Called lazily/in the background because each
/// call spawns a `pkgutil` process (~0.25s).
pub fn owner_of(path: &str, pku: &dyn PkgUtil) -> Option<String> {
    pku.file_info(path).unwrap_or_default().into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockPkgUtil {
        owners: HashMap<String, Vec<String>>,
    }
    impl PkgUtil for MockPkgUtil {
        fn file_info(&self, path: &str) -> Option<Vec<String>> {
            Some(self.owners.get(path).cloned().unwrap_or_default())
        }
        fn files(&self, _pkg_id: &str) -> Option<Vec<String>> {
            None
        }
        fn install_root(&self, _pkg_id: &str) -> Option<String> {
            None
        }
    }

    #[test]
    fn owner_of_returns_first_owner_or_none() {
        let pku = MockPkgUtil {
            owners: HashMap::from([("/A.vst3".to_string(), vec!["com.acme.pkg".to_string()])]),
        };
        assert_eq!(owner_of("/A.vst3", &pku).as_deref(), Some("com.acme.pkg"));
        assert_eq!(owner_of("/orphan.vst3", &pku), None);
    }

    struct FailingPkgUtil;
    impl PkgUtil for FailingPkgUtil {
        fn file_info(&self, _path: &str) -> Option<Vec<String>> {
            None
        }
        fn files(&self, _pkg_id: &str) -> Option<Vec<String>> {
            None
        }
        fn install_root(&self, _pkg_id: &str) -> Option<String> {
            None
        }
    }

    #[test]
    fn owner_of_none_when_pkgutil_unavailable() {
        assert_eq!(owner_of("/x.vst3", &FailingPkgUtil), None);
    }

    #[test]
    fn join_root_handles_volume_and_location_shapes() {
        assert_eq!(join_root("/", "/"), "/");
        assert_eq!(join_root("/", "."), "/");
        assert_eq!(join_root("/", ""), "/");
        assert_eq!(join_root("/", "Applications"), "/Applications");
        assert_eq!(join_root("/", "./Applications"), "/Applications");
        assert_eq!(join_root("/Volumes/HD", "/"), "/Volumes/HD");
        assert_eq!(join_root("/Volumes/HD", "Library"), "/Volumes/HD/Library");
    }
}
