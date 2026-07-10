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
}
