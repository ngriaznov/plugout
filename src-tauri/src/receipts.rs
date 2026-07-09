use std::process::Command;

pub trait PkgUtil {
    /// Package ids that claim ownership of `path`. `None` = pkgutil could not run.
    fn file_info(&self, path: &str) -> Option<Vec<String>>;
}

pub struct RealPkgUtil;

impl PkgUtil for RealPkgUtil {
    fn file_info(&self, path: &str) -> Option<Vec<String>> {
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
    }

    #[test]
    fn owner_of_none_when_pkgutil_unavailable() {
        assert_eq!(owner_of("/x.vst3", &FailingPkgUtil), None);
    }
}
