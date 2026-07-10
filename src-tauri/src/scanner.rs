use crate::model::{Format, PluginBundle, Scope};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct PluginMeta {
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub bundle_id: String,
}

pub fn parse_info_plist(path: &Path) -> PluginMeta {
    let Ok(value) = plist::Value::from_file(path) else {
        return PluginMeta::default();
    };
    let Some(dict) = value.as_dictionary() else {
        return PluginMeta::default();
    };
    let get = |k: &str| dict.get(k).and_then(|v| v.as_string()).map(str::to_string);

    let name = get("CFBundleName")
        .or_else(|| get("CFBundleDisplayName"))
        .unwrap_or_default();
    let bundle_id = get("CFBundleIdentifier").unwrap_or_default();
    let version = get("CFBundleShortVersionString")
        .or_else(|| get("CFBundleVersion"))
        .unwrap_or_default();
    let vendor = audio_component_vendor(dict).unwrap_or_else(|| vendor_from_bundle_id(&bundle_id));

    PluginMeta {
        name,
        vendor,
        version,
        bundle_id,
    }
}

fn audio_component_vendor(dict: &plist::Dictionary) -> Option<String> {
    let name = dict
        .get("AudioComponents")?
        .as_array()?
        .first()?
        .as_dictionary()?
        .get("name")?
        .as_string()?;
    let vendor = name.split(':').next()?.trim();
    (!vendor.is_empty()).then(|| vendor.to_string())
}

fn vendor_from_bundle_id(bundle_id: &str) -> String {
    let parts: Vec<&str> = bundle_id.split('.').collect();
    let tlds = ["com", "net", "org", "io", "co", "app"];
    let idx = if parts.len() > 1 && tlds.contains(&parts[0]) {
        1
    } else {
        0
    };
    parts.get(idx).unwrap_or(&"").to_string()
}

pub fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if meta.is_file() {
        return meta.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries.flatten().map(|e| dir_size(&e.path())).sum()
}

pub fn scan_dir(dir: &Path, format: Format, scope: Scope) -> Vec<PluginBundle> {
    let want = format.extension();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(want) {
            continue;
        }
        let meta = parse_info_plist(&path.join("Contents/Info.plist"));
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let name = if meta.name.is_empty() {
            file_stem
        } else {
            meta.name
        };
        let path_str = path.to_string_lossy().to_string();
        out.push(PluginBundle {
            id: path_str.clone(),
            name,
            vendor: meta.vendor,
            version: meta.version,
            format,
            bundle_id: meta.bundle_id,
            path: path_str,
            size_bytes: dir_size(&path),
            scope,
            package_id: None,
        });
    }
    out
}

/// Application folders to search for companion apps.
pub fn application_roots() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    vec![
        PathBuf::from("/Applications"),
        PathBuf::from(format!("{home}/Applications")),
    ]
}

/// The vendor segment of a reverse-DNS bundle id, lowercased:
/// "com.Arturia.asc2" → "arturia". Used to tie apps to plugin vendors.
/// Apple is excluded: macOS ships stock audio plugins, and matching on
/// "apple" would claim Safari, Xcode and every other system app.
fn vendor_key(bundle_id: &str) -> Option<String> {
    let mut parts = bundle_id.split('.');
    let _tld = parts.next()?;
    let vendor = parts.next()?;
    (!vendor.is_empty() && !vendor.eq_ignore_ascii_case("apple")).then(|| vendor.to_lowercase())
}

/// Enumerate companion apps by walking the application roots (top level plus
/// one vendor folder deep) and keeping every `.app` that matches the scanned
/// plugins by name, by bundle-id vendor, or by vendor-named parent folder.
/// A filesystem walk is instant, where deriving apps from installer receipts
/// takes minutes of `pkgutil` calls — receipts trail in afterwards to fill
/// the "Installed by" field.
pub fn scan_applications(roots: &[PathBuf], plugins: &[PluginBundle]) -> Vec<PluginBundle> {
    let plugin_names: std::collections::BTreeMap<String, &PluginBundle> =
        plugins.iter().map(|p| (p.name.to_lowercase(), p)).collect();
    let vendor_keys: std::collections::BTreeMap<String, &PluginBundle> = plugins
        .iter()
        .filter_map(|p| vendor_key(&p.bundle_id).map(|k| (k, p)))
        .collect();
    let vendor_names: std::collections::BTreeMap<String, &PluginBundle> = plugins
        .iter()
        .map(|p| (p.vendor.to_lowercase(), p))
        .collect();
    let home = std::env::var("HOME").unwrap_or_default();

    let mut out = Vec::new();
    for root in roots {
        for (path, parent_dir) in apps_under(root) {
            let meta = parse_info_plist(&path.join("Contents/Info.plist"));
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let name = if meta.name.is_empty() {
                stem.clone()
            } else {
                meta.name.clone()
            };

            let linked = plugin_names
                .get(&name.to_lowercase())
                .or_else(|| plugin_names.get(&stem.to_lowercase()))
                .or_else(|| vendor_key(&meta.bundle_id).and_then(|k| vendor_keys.get(&k)))
                .or_else(|| {
                    parent_dir
                        .as_ref()
                        .and_then(|d| vendor_names.get(&d.to_lowercase()))
                });
            let Some(linked) = linked else {
                continue;
            };

            let path_str = path.to_string_lossy().to_string();
            out.push(PluginBundle {
                id: path_str.clone(),
                name,
                vendor: linked.vendor.clone(),
                version: meta.version,
                format: Format::App,
                bundle_id: meta.bundle_id,
                path: path_str.clone(),
                size_bytes: dir_size(&path),
                scope: if path_str.starts_with(&format!("{home}/")) {
                    Scope::User
                } else {
                    Scope::System
                },
                package_id: None, // receipts trail in via receipt:update
            });
        }
    }
    out
}

/// `.app` bundles directly under `root`, plus those one subfolder deep
/// (vendor folders like /Applications/Arturia). Yields (app path, parent
/// folder name when nested).
fn apps_under(root: &Path) -> Vec<(PathBuf, Option<String>)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_app = path.extension().and_then(|e| e.to_str()) == Some("app");
        if is_app {
            out.push((path, None));
        } else if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .map(str::to_string);
            if let Ok(children) = std::fs::read_dir(&path) {
                for child in children.flatten() {
                    let cpath = child.path();
                    if cpath.extension().and_then(|e| e.to_str()) == Some("app") {
                        out.push((cpath, dir_name.clone()));
                    }
                }
            }
        }
    }
    out
}

pub fn plugin_locations() -> Vec<(PathBuf, Format, Scope)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let u = |p: &str| PathBuf::from(format!("{home}/Library/Audio/Plug-Ins/{p}"));
    let s = |p: &str| PathBuf::from(format!("/Library/Audio/Plug-Ins/{p}"));
    vec![
        (u("Components"), Format::Au, Scope::User),
        (s("Components"), Format::Au, Scope::System),
        (u("VST3"), Format::Vst3, Scope::User),
        (s("VST3"), Format::Vst3, Scope::System),
        (u("VST"), Format::Vst2, Scope::User),
        (s("VST"), Format::Vst2, Scope::System),
        (u("CLAP"), Format::Clap, Scope::User),
        (s("CLAP"), Format::Clap, Scope::System),
        (
            PathBuf::from("/Library/Application Support/Avid/Audio/Plug-Ins"),
            Format::Aax,
            Scope::System,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_plist(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("Info.plist");
        let mut f = std::fs::File::create(&p).unwrap();
        write!(
            f,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>{}</dict></plist>",
            body
        ).unwrap();
        p
    }

    #[test]
    fn reads_name_version_bundleid() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_plist(
            dir.path(),
            "<key>CFBundleName</key><string>Massive</string>\
             <key>CFBundleShortVersionString</key><string>1.5.9</string>\
             <key>CFBundleIdentifier</key><string>com.native-instruments.Massive</string>",
        );
        let m = parse_info_plist(&p);
        assert_eq!(m.name, "Massive");
        assert_eq!(m.version, "1.5.9");
        assert_eq!(m.bundle_id, "com.native-instruments.Massive");
        assert_eq!(m.vendor, "native-instruments");
    }

    #[test]
    fn vendor_from_audio_components_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_plist(
            dir.path(),
            "<key>CFBundleIdentifier</key><string>com.acme.x</string>\
             <key>AudioComponents</key><array><dict>\
             <key>name</key><string>FabFilter: Pro-Q 3</string></dict></array>",
        );
        let m = parse_info_plist(&p);
        assert_eq!(m.vendor, "FabFilter");
    }

    use crate::model::{Format, Scope};

    fn make_bundle(root: &Path, name: &str, plist_body: &str) {
        let contents = root.join(name).join("Contents");
        std::fs::create_dir_all(&contents).unwrap();
        let mut f = std::fs::File::create(contents.join("Info.plist")).unwrap();
        write!(f,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>{}</dict></plist>",
            plist_body).unwrap();
        // a payload file so size > 0
        std::fs::write(contents.join("payload.bin"), vec![0u8; 2048]).unwrap();
    }

    #[test]
    fn scan_dir_finds_matching_bundles_only() {
        let dir = tempfile::tempdir().unwrap();
        make_bundle(dir.path(), "Alpha.vst3",
            "<key>CFBundleName</key><string>Alpha</string><key>CFBundleIdentifier</key><string>com.x.alpha</string>");
        make_bundle(
            dir.path(),
            "Beta.component",
            "<key>CFBundleName</key><string>Beta</string>",
        );
        std::fs::write(dir.path().join("notes.txt"), b"nope").unwrap();

        let found = scan_dir(dir.path(), Format::Vst3, Scope::User);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Alpha");
        assert_eq!(found[0].format, Format::Vst3);
        assert_eq!(found[0].scope, Scope::User);
        assert!(found[0].size_bytes >= 2048, "size {}", found[0].size_bytes);
        assert_eq!(found[0].id, found[0].path);
    }

    #[test]
    fn scan_dir_falls_back_to_filename_when_no_name() {
        let dir = tempfile::tempdir().unwrap();
        make_bundle(dir.path(), "NoName.vst3", "");
        let found = scan_dir(dir.path(), Format::Vst3, Scope::User);
        assert_eq!(found[0].name, "NoName");
    }

    #[test]
    fn scan_dir_missing_directory_is_empty_not_error() {
        let found = scan_dir(Path::new("/definitely/not/here"), Format::Au, Scope::System);
        assert!(found.is_empty());
    }

    fn make_app(root: &Path, rel: &str, plist_body: &str) {
        let contents = root.join(rel).join("Contents");
        std::fs::create_dir_all(&contents).unwrap();
        let mut f = std::fs::File::create(contents.join("Info.plist")).unwrap();
        write!(f,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>{}</dict></plist>",
            plist_body).unwrap();
    }

    fn plugin(name: &str, vendor: &str, bundle_id: &str) -> PluginBundle {
        PluginBundle {
            id: format!("/L/{name}.vst3"),
            name: name.into(),
            vendor: vendor.into(),
            version: "1".into(),
            format: Format::Vst3,
            bundle_id: bundle_id.into(),
            path: format!("/L/{name}.vst3"),
            size_bytes: 1,
            scope: Scope::System,
            package_id: None,
        }
    }

    #[test]
    fn scan_applications_matches_by_name_vendor_id_and_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // name match, top level
        make_app(
            root,
            "Serum.app",
            "<key>CFBundleIdentifier</key><string>com.xferrecords.serum</string>",
        );
        // bundle-id vendor match (a vendor tool, name matches no plugin)
        make_app(
            root,
            "Arturia Software Center.app",
            "<key>CFBundleName</key><string>Arturia Software Center</string>\
             <key>CFBundleIdentifier</key><string>com.Arturia.asc2</string>",
        );
        // vendor-folder match, nested
        make_app(
            root,
            "Arturia/Pigments.app",
            "<key>CFBundleIdentifier</key><string>com.Arturia.Pigments</string>",
        );
        // unrelated app: no match
        make_app(
            root,
            "Safari.app",
            "<key>CFBundleIdentifier</key><string>com.apple.Safari</string>",
        );

        let plugins = vec![
            plugin("Serum", "Xfer Records", "com.xferrecords.serum.vst3"),
            plugin("Pigments", "Arturia", "com.Arturia.Pigments.vst3"),
        ];
        let mut apps = scan_applications(&[root.to_path_buf()], &plugins);
        apps.sort_by(|a, b| a.name.cmp(&b.name));

        let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Arturia Software Center", "Pigments", "Serum"]);
        assert!(apps.iter().all(|a| a.format == Format::App));
        // vendor inherited from the linked plugin so rows merge
        assert_eq!(apps[1].vendor, "Arturia");
        assert_eq!(apps[2].vendor, "Xfer Records");
    }

    #[test]
    fn scan_applications_ignores_everything_without_a_link() {
        let dir = tempfile::tempdir().unwrap();
        make_app(
            dir.path(),
            "Notes.app",
            "<key>CFBundleIdentifier</key><string>com.apple.Notes</string>",
        );
        let plugins = vec![plugin(
            "Serum",
            "Xfer Records",
            "com.xferrecords.serum.vst3",
        )];
        assert!(scan_applications(&[dir.path().to_path_buf()], &plugins).is_empty());
    }
}
