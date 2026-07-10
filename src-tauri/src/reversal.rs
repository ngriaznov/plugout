//! Receipt-based reversal: what else did a plugin's installer put on this
//! machine? Companion apps become APP-format bundles; everything else is
//! offered as removable "support files" — but only under three guards:
//! receipts are the sole evidence, a package must be exclusive to the removal
//! (no surviving plugin may share it), and candidates must sit under an
//! allowlist of safe roots. Directories are offered whole only when their
//! entire on-disk contents are receipt-owned ("subtree proof").

use crate::receipts::PkgUtil;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Absolute paths a package wrote, resolved against its install root.
pub fn package_paths(pkg_id: &str, pku: &dyn PkgUtil) -> Vec<String> {
    let Some(files) = pku.files(pkg_id) else {
        return Vec::new();
    };
    let root = pku.install_root(pkg_id).unwrap_or_else(|| "/".to_string());
    let prefix = if root == "/" { String::new() } else { root };
    files
        .iter()
        .map(|rel| format!("{prefix}/{}", rel.trim_start_matches('/')))
        .collect()
}

/// Top-level `.app` bundles among a package's paths (the app itself, not files
/// inside it): `/Applications/X.app`, `~/Applications/X.app`.
pub fn apps_in(paths: &[String]) -> Vec<String> {
    let mut apps: BTreeSet<String> = BTreeSet::new();
    for p in paths {
        if let Some(idx) = p.find("Applications/") {
            let after = &p[idx + "Applications/".len()..];
            if let Some(app) = after.split('/').next() {
                if let Some(stem) = app.strip_suffix(".app") {
                    if !stem.is_empty() {
                        apps.insert(format!("{}{app}", &p[..idx + "Applications/".len()]));
                    }
                }
            }
        }
    }
    apps.into_iter().collect()
}

/// Roots under which support files may be offered for removal, relative to
/// either Library root. Anything an installer wrote elsewhere is never touched.
const ALLOWED_UNDER_LIBRARY: &[&str] = &[
    "Application Support/",
    "Preferences/",
    "Caches/",
    "Audio/Presets/",
];

fn is_allowlisted(path: &str) -> bool {
    if path.starts_with("/Users/Shared/") {
        return true;
    }
    let under_library = path
        .strip_prefix("/Library/")
        .or_else(|| path.split_once("/Library/").map(|(_, rest)| rest));
    match under_library {
        Some(rest) => ALLOWED_UNDER_LIBRARY.iter().any(|a| rest.starts_with(a)),
        None => false,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SupportFile {
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPreview {
    pub support_files: Vec<SupportFile>,
    /// Packages skipped because a surviving plugin still uses them.
    pub skipped_shared: u32,
}

/// Filesystem probe used by the roll-up logic; mocked in tests.
pub trait Fs {
    fn exists(&self, path: &str) -> bool;
    /// Direct children of a directory (absolute paths); empty if not a dir.
    fn children(&self, path: &str) -> Vec<String>;
    fn size(&self, path: &str) -> u64;
}

pub struct RealFs;
impl Fs for RealFs {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    fn children(&self, path: &str) -> Vec<String> {
        std::fs::read_dir(path)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn size(&self, path: &str) -> u64 {
        crate::scanner::dir_size(Path::new(path))
    }
}

/// Compute the support files offered when removing `removing` (bundle paths,
/// apps included). `owner_of_removing` maps each removing path to its package
/// (if any); `all_owned` maps every scanned bundle path (including survivors)
/// to its package.
pub fn removal_preview(
    removing: &BTreeSet<String>,
    all_owned: &BTreeMap<String, String>,
    pku: &dyn PkgUtil,
    fs: &dyn Fs,
) -> RemovalPreview {
    // Packages involved in this removal.
    let packages: BTreeSet<&String> = removing.iter().filter_map(|p| all_owned.get(p)).collect();

    // Exclusivity: drop packages that also own a bundle NOT being removed.
    let mut skipped_shared = 0u32;
    let exclusive: Vec<&String> = packages
        .into_iter()
        .filter(|pkg| {
            let shared = all_owned
                .iter()
                .any(|(path, owner)| owner == *pkg && !removing.contains(path));
            if shared {
                skipped_shared += 1;
            }
            !shared
        })
        .collect();

    // Candidate paths: allowlisted, on disk, not part of the removal itself.
    let mut owned: BTreeSet<String> = BTreeSet::new();
    for pkg in exclusive {
        for path in package_paths(pkg, pku) {
            let inside_removed = removing
                .iter()
                .any(|r| path == *r || path.starts_with(&format!("{r}/")));
            if is_allowlisted(&path) && !inside_removed && fs.exists(&path) {
                owned.insert(path);
            }
        }
    }

    // Roll up: keep a path only if no owned ancestor fully covers it; a
    // directory covers itself when every on-disk child is owned or covered.
    let mut keep: Vec<String> = Vec::new();
    'outer: for path in &owned {
        let mut ancestor = Path::new(path).parent();
        while let Some(a) = ancestor {
            let a_str = a.to_string_lossy();
            if owned.contains(a_str.as_ref()) && fully_owned(&a_str, &owned, fs) {
                continue 'outer; // an ancestor will carry it
            }
            ancestor = a.parent();
        }
        if !fully_owned(path, &owned, fs) && !fs.children(path).is_empty() {
            continue; // partially-owned directory: its owned children stand alone
        }
        keep.push(path.clone());
    }

    RemovalPreview {
        support_files: keep
            .into_iter()
            .map(|path| SupportFile {
                size_bytes: fs.size(&path),
                path,
            })
            .collect(),
        skipped_shared,
    }
}

/// True when every on-disk child of `path` is owned (recursively) — the
/// "subtree proof" that lets a directory be offered whole.
fn fully_owned(path: &str, owned: &BTreeSet<String>, fs: &dyn Fs) -> bool {
    fs.children(path).iter().all(|child| {
        owned.contains(child) && (fs.children(child).is_empty() || fully_owned(child, owned, fs))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockPku {
        files: HashMap<String, Vec<String>>,
        roots: HashMap<String, String>,
    }
    impl PkgUtil for MockPku {
        fn file_info(&self, _p: &str) -> Option<Vec<String>> {
            None
        }
        fn files(&self, pkg: &str) -> Option<Vec<String>> {
            self.files.get(pkg).cloned()
        }
        fn install_root(&self, pkg: &str) -> Option<String> {
            self.roots.get(pkg).cloned()
        }
    }

    /// Every listed path exists; directories are paths listed with children.
    struct MockFs {
        tree: HashMap<String, Vec<String>>,
        all: BTreeSet<String>,
    }
    impl MockFs {
        fn new(paths: &[&str]) -> Self {
            let mut tree: HashMap<String, Vec<String>> = HashMap::new();
            let all: BTreeSet<String> = paths.iter().map(|s| s.to_string()).collect();
            for p in paths {
                if let Some(parent) = Path::new(p).parent() {
                    let parent = parent.to_string_lossy().into_owned();
                    if all.contains(&parent) {
                        tree.entry(parent).or_default().push(p.to_string());
                    }
                }
            }
            Self { tree, all }
        }
    }
    impl Fs for MockFs {
        fn exists(&self, path: &str) -> bool {
            self.all.contains(path)
        }
        fn children(&self, path: &str) -> Vec<String> {
            self.tree.get(path).cloned().unwrap_or_default()
        }
        fn size(&self, _path: &str) -> u64 {
            10
        }
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn package_paths_resolve_against_install_root() {
        let pku = MockPku {
            files: HashMap::from([(
                "com.x".to_string(),
                vec![
                    "Applications/X.app".to_string(),
                    "Library/Preferences/x.plist".to_string(),
                ],
            )]),
            roots: HashMap::from([("com.x".to_string(), "/".to_string())]),
        };
        assert_eq!(
            package_paths("com.x", &pku),
            vec!["/Applications/X.app", "/Library/Preferences/x.plist"]
        );
    }

    #[test]
    fn apps_in_finds_top_level_apps_only() {
        let paths = vec![
            "/Applications/Serum.app".to_string(),
            "/Applications/Serum.app/Contents/Info.plist".to_string(),
            "/Users/me/Applications/Mini.app/Contents/MacOS/mini".to_string(),
            "/Library/Application Support/X/notanapp".to_string(),
        ];
        assert_eq!(
            apps_in(&paths),
            vec!["/Applications/Serum.app", "/Users/me/Applications/Mini.app"]
        );
    }

    #[test]
    fn allowlist_covers_both_library_roots_and_users_shared() {
        assert!(is_allowlisted("/Library/Application Support/X/f"));
        assert!(is_allowlisted("/Users/me/Library/Preferences/com.x.plist"));
        assert!(is_allowlisted("/Library/Caches/com.x"));
        assert!(is_allowlisted("/Users/Shared/X/f"));
        assert!(!is_allowlisted("/usr/local/bin/x"));
        assert!(!is_allowlisted("/Library/Audio/Plug-Ins/VST3/X.vst3"));
        assert!(!is_allowlisted("/Library/LaunchDaemons/com.x.plist"));
    }

    #[test]
    fn shared_package_is_skipped_entirely() {
        let pku = MockPku {
            files: HashMap::from([(
                "com.v.suite".to_string(),
                vec!["Library/Application Support/V/data".to_string()],
            )]),
            roots: HashMap::new(),
        };
        let fs = MockFs::new(&["/Library/Application Support/V/data"]);
        // The suite package owns a bundle that is NOT being removed.
        let all_owned = BTreeMap::from([
            (
                "/Library/Audio/Plug-Ins/VST3/A.vst3".to_string(),
                "com.v.suite".to_string(),
            ),
            (
                "/Library/Audio/Plug-Ins/VST3/B.vst3".to_string(),
                "com.v.suite".to_string(),
            ),
        ]);
        let preview = removal_preview(
            &set(&["/Library/Audio/Plug-Ins/VST3/A.vst3"]),
            &all_owned,
            &pku,
            &fs,
        );
        assert!(preview.support_files.is_empty());
        assert_eq!(preview.skipped_shared, 1);
    }

    #[test]
    fn exclusive_package_offers_allowlisted_files() {
        let pku = MockPku {
            files: HashMap::from([(
                "com.x.pkg".to_string(),
                vec![
                    "Library/Application Support/X/presets.bank".to_string(),
                    "usr/local/lib/x.dylib".to_string(), // not allowlisted
                    "Library/Preferences/gone.plist".to_string(), // not on disk
                ],
            )]),
            roots: HashMap::new(),
        };
        let fs = MockFs::new(&["/Library/Application Support/X/presets.bank"]);
        let all_owned = BTreeMap::from([(
            "/Library/Audio/Plug-Ins/VST3/X.vst3".to_string(),
            "com.x.pkg".to_string(),
        )]);
        let preview = removal_preview(
            &set(&["/Library/Audio/Plug-Ins/VST3/X.vst3"]),
            &all_owned,
            &pku,
            &fs,
        );
        assert_eq!(
            preview.support_files,
            vec![SupportFile {
                path: "/Library/Application Support/X/presets.bank".into(),
                size_bytes: 10
            }]
        );
        assert_eq!(preview.skipped_shared, 0);
    }

    #[test]
    fn fully_owned_directory_rolls_up_to_one_entry() {
        let pku = MockPku {
            files: HashMap::from([(
                "com.x.pkg".to_string(),
                vec![
                    "Library/Application Support/X".to_string(),
                    "Library/Application Support/X/a".to_string(),
                    "Library/Application Support/X/b".to_string(),
                ],
            )]),
            roots: HashMap::new(),
        };
        let fs = MockFs::new(&[
            "/Library/Application Support/X",
            "/Library/Application Support/X/a",
            "/Library/Application Support/X/b",
        ]);
        let all_owned = BTreeMap::from([(
            "/Library/Audio/Plug-Ins/VST3/X.vst3".to_string(),
            "com.x.pkg".to_string(),
        )]);
        let preview = removal_preview(
            &set(&["/Library/Audio/Plug-Ins/VST3/X.vst3"]),
            &all_owned,
            &pku,
            &fs,
        );
        let paths: Vec<&str> = preview
            .support_files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(paths, vec!["/Library/Application Support/X"]);
    }

    #[test]
    fn partially_owned_directory_offers_only_owned_children() {
        let pku = MockPku {
            files: HashMap::from([(
                "com.x.pkg".to_string(),
                vec![
                    "Library/Application Support/Shared".to_string(),
                    "Library/Application Support/Shared/mine".to_string(),
                ],
            )]),
            roots: HashMap::new(),
        };
        // On disk the directory also holds a foreign file the package never wrote.
        let fs = MockFs::new(&[
            "/Library/Application Support/Shared",
            "/Library/Application Support/Shared/mine",
            "/Library/Application Support/Shared/other-vendors-file",
        ]);
        let all_owned = BTreeMap::from([(
            "/Library/Audio/Plug-Ins/VST3/X.vst3".to_string(),
            "com.x.pkg".to_string(),
        )]);
        let preview = removal_preview(
            &set(&["/Library/Audio/Plug-Ins/VST3/X.vst3"]),
            &all_owned,
            &pku,
            &fs,
        );
        let paths: Vec<&str> = preview
            .support_files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(paths, vec!["/Library/Application Support/Shared/mine"]);
    }

    #[test]
    fn paths_inside_removed_bundles_are_not_double_offered() {
        let pku = MockPku {
            files: HashMap::from([(
                "com.x.pkg".to_string(),
                vec!["Library/Application Support/X/f".to_string()],
            )]),
            roots: HashMap::new(),
        };
        let fs = MockFs::new(&["/Library/Application Support/X/f"]);
        let all_owned = BTreeMap::from([(
            "/Library/Application Support/X/f".to_string(),
            "com.x.pkg".to_string(),
        )]);
        // The "bundle" being removed IS that path (e.g. an APP entry) — nothing extra.
        let preview = removal_preview(
            &set(&["/Library/Application Support/X/f"]),
            &all_owned,
            &pku,
            &fs,
        );
        assert!(preview.support_files.is_empty());
    }
}
