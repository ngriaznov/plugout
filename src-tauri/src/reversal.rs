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

/// Installers usually split one product into many receipts —
/// `com.Arturia.ARP2600V3.{vst3,au,standalone,resources,…}`. The family prefix
/// groups them: the id minus its last segment, required to keep at least three
/// segments so a whole vendor (`com.arturia.*`) is never grouped.
pub fn family_prefix(pkg_id: &str) -> Option<String> {
    let cut = pkg_id.rfind('.')?;
    let prefix = &pkg_id[..cut];
    (prefix.matches('.').count() >= 2).then(|| format!("{prefix}."))
}

/// All receipts in `pkg_id`'s family (always includes `pkg_id` itself).
pub fn expand_family(pkg_id: &str, all_pkgs: &[String]) -> BTreeSet<String> {
    let mut family = BTreeSet::from([pkg_id.to_string()]);
    if let Some(prefix) = family_prefix(pkg_id) {
        family.extend(all_pkgs.iter().filter(|p| p.starts_with(&prefix)).cloned());
    }
    family
}

/// `.app` bundles among a package's paths, at any depth under an Applications
/// folder: `/Applications/X.app`, `/Applications/Arturia/X.app`, `~/Applications/…`.
/// Returns the app bundle path itself, not files inside it.
pub fn apps_in(paths: &[String]) -> Vec<String> {
    let mut apps: BTreeSet<String> = BTreeSet::new();
    for p in paths {
        let Some(idx) = p.find("Applications/") else {
            continue;
        };
        let base = &p[..idx + "Applications/".len()];
        let mut acc = String::new();
        for comp in p[base.len()..].split('/') {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(comp);
            if comp.len() > 4 && comp.ends_with(".app") {
                apps.insert(format!("{base}{acc}"));
                break;
            }
        }
    }
    apps.into_iter().collect()
}

/// Known-safe roots under either Library root. Vendors also write directly to
/// `Library/<Vendor>/…` (Arturia, UVI), so paths at least one level inside any
/// Library directory are allowed unless the directory is system-critical.
const ALLOWED_UNDER_LIBRARY: &[&str] = &[
    "Application Support/",
    "Preferences/",
    "Caches/",
    "Audio/Presets/",
];

/// Library subtrees never offered, regardless of receipts.
const DENIED_UNDER_LIBRARY: &[&str] = &[
    "Audio/", // plugin folders themselves (Presets excepted above)
    "Frameworks/",
    "Extensions/",
    "SystemExtensions/",
    "LaunchAgents/",
    "LaunchDaemons/",
    "PrivilegedHelperTools/",
    "PreferencePanes/",
    "StartupItems/",
    "Security/",
    "Keychains/",
    "Fonts/",
    "Input Methods/",
    "QuickLook/",
    "Screen Savers/",
    "Services/",
    "Spotlight/",
    "WebServer/",
    "Developer/",
];

fn is_allowlisted(path: &str) -> bool {
    if path.starts_with("/Users/Shared/") {
        return true;
    }
    let under_library = path
        .strip_prefix("/Library/")
        .or_else(|| path.split_once("/Library/").map(|(_, rest)| rest));
    let Some(rest) = under_library else {
        return false;
    };
    if ALLOWED_UNDER_LIBRARY.iter().any(|a| rest.starts_with(a)) {
        return true;
    }
    if DENIED_UNDER_LIBRARY.iter().any(|d| rest.starts_with(d)) {
        return false;
    }
    // Vendor-directory rule: allow contents of Library/<Vendor>/…, but never the
    // vendor directory itself — only subtree-proven children roll up.
    rest.contains('/') && !rest.ends_with('/')
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
    // Package families involved in this removal (one product = many receipts).
    let all_pkgs = pku.all_packages();
    let owner_pkgs: BTreeSet<&String> = removing.iter().filter_map(|p| all_owned.get(p)).collect();

    // Exclusivity per family: skip the whole family when any of its receipts
    // owns a bundle that is staying installed.
    let mut skipped_shared = 0u32;
    let mut exclusive: BTreeSet<String> = BTreeSet::new();
    for pkg in owner_pkgs {
        let family = expand_family(pkg, &all_pkgs);
        let shared = all_owned
            .iter()
            .any(|(path, owner)| family.contains(owner) && !removing.contains(path));
        if shared {
            skipped_shared += 1;
        } else {
            exclusive.extend(family);
        }
    }

    // Candidate paths: allowlisted, on disk, not part of the removal itself.
    let mut owned: BTreeSet<String> = BTreeSet::new();
    for pkg in &exclusive {
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
        fn all_packages(&self) -> Vec<String> {
            self.files.keys().cloned().collect()
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
    fn apps_in_finds_apps_at_any_depth_under_applications() {
        let paths = vec![
            "/Applications/Serum.app".to_string(),
            "/Applications/Serum.app/Contents/Info.plist".to_string(),
            "/Applications/Arturia/ARP 2600 V3.app/Contents".to_string(),
            "/Users/me/Applications/Mini.app/Contents/MacOS/mini".to_string(),
            "/Applications/Arturia".to_string(), // bare vendor dir: not an app
            "/Library/Application Support/X/notanapp".to_string(),
        ];
        assert_eq!(
            apps_in(&paths),
            vec![
                "/Applications/Arturia/ARP 2600 V3.app",
                "/Applications/Serum.app",
                "/Users/me/Applications/Mini.app"
            ]
        );
    }

    #[test]
    fn family_prefix_needs_three_segments() {
        assert_eq!(
            family_prefix("com.Arturia.ARP2600V3.vst3").as_deref(),
            Some("com.Arturia.ARP2600V3.")
        );
        assert_eq!(family_prefix("com.soundtoys.all"), None); // would group the vendor
        assert_eq!(family_prefix("nodots"), None);
    }

    #[test]
    fn expand_family_groups_sibling_receipts() {
        let all = vec![
            "com.Arturia.ARP2600V3.vst3".to_string(),
            "com.Arturia.ARP2600V3.standalone".to_string(),
            "com.Arturia.ARP2600V3.resources".to_string(),
            "com.Arturia.AcidV.vst3".to_string(),
        ];
        let family = expand_family("com.Arturia.ARP2600V3.vst3", &all);
        assert!(family.contains("com.Arturia.ARP2600V3.standalone"));
        assert!(family.contains("com.Arturia.ARP2600V3.resources"));
        assert!(!family.contains("com.Arturia.AcidV.vst3"));
    }

    #[test]
    fn family_shared_with_surviving_format_is_skipped() {
        // ARP's VST3 is removed but its AU stays: the whole family (including
        // the standalone/resources receipts) must be kept back.
        let pku = MockPku {
            files: HashMap::from([
                ("com.a.arp.vst3".to_string(), vec![]),
                ("com.a.arp.au".to_string(), vec![]),
                (
                    "com.a.arp.resources".to_string(),
                    vec!["Library/A/ARP/presets".to_string()],
                ),
            ]),
            roots: HashMap::new(),
        };
        let fs = MockFs::new(&["/Library/A/ARP/presets"]);
        let all_owned = BTreeMap::from([
            ("/L/VST3/ARP.vst3".to_string(), "com.a.arp.vst3".to_string()),
            (
                "/L/Components/ARP.component".to_string(),
                "com.a.arp.au".to_string(),
            ),
        ]);
        let preview = removal_preview(&set(&["/L/VST3/ARP.vst3"]), &all_owned, &pku, &fs);
        assert!(preview.support_files.is_empty());
        assert_eq!(preview.skipped_shared, 1);

        // Removing both formats frees the family.
        let preview = removal_preview(
            &set(&["/L/VST3/ARP.vst3", "/L/Components/ARP.component"]),
            &all_owned,
            &pku,
            &fs,
        );
        assert_eq!(preview.support_files.len(), 1);
        assert_eq!(preview.support_files[0].path, "/Library/A/ARP/presets");
    }

    #[test]
    fn vendor_directory_rule() {
        assert!(is_allowlisted("/Library/Arturia/ARP 2600 V3/presets.bank"));
        assert!(is_allowlisted("/Users/me/Library/UVI/Falcon/sounds"));
        assert!(!is_allowlisted("/Library/Arturia")); // vendor root itself: never whole
        assert!(!is_allowlisted("/Library/Frameworks/Vendor.framework/f"));
        assert!(!is_allowlisted("/Library/LaunchDaemons/com.x.plist"));
        assert!(!is_allowlisted("/Library/Audio/Plug-Ins/VST3/X.vst3"));
        assert!(is_allowlisted(
            "/Library/Audio/Presets/Vendor/preset.aupreset"
        ));
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

    /// Real-system probe (requires installed Arturia plugins; not run in CI):
    /// `cargo test real_system -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_system_discovery_probe() {
        use crate::receipts::RealPkgUtil;
        let plugin = "/Library/Audio/Plug-Ins/VST3/ARP 2600 V3.vst3";
        let owner = crate::receipts::owner_of(plugin, &RealPkgUtil).expect("owner");
        let all = RealPkgUtil.all_packages();
        let family = expand_family(&owner, &all);
        println!("owner={owner} family={family:?}");
        assert!(family.len() > 1, "family should have sibling receipts");

        let apps: Vec<String> = family
            .iter()
            .flat_map(|p| apps_in(&package_paths(p, &RealPkgUtil)))
            .collect();
        println!("apps={apps:?}");
        assert!(
            apps.iter().any(|a| a.contains("ARP 2600 V3.app")),
            "should find the nested standalone app"
        );
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
