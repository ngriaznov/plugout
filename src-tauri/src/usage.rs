//! DAW project usage: find REAPER (.rpp) and Ableton (.als) project files and
//! extract which plugins they reference. Discovery shells out to Spotlight
//! behind [`ProjectFinder`] (walk fallback when mdfind fails), so parsing is
//! testable without the real filesystem. Per-file failures are skipped — a
//! corrupt project must never abort the batch.
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct PluginRef {
    pub name: String,
    pub vendor: String, // may be empty (VST2 blocks in .als carry no vendor)
}

pub trait ProjectFinder {
    fn mdfind(&self, query: &str) -> Result<Vec<PathBuf>, String>;
    fn walk_fallback(&self) -> Vec<PathBuf>;
}

/// First quoted string of a `<VST/AU/CLAP ...>` line: `FORMAT: Name (Vendor)`.
fn split_ref(label: &str) -> Option<PluginRef> {
    let rest = label.split_once(": ").map_or(label, |(_, r)| r);
    let (name, vendor) = match rest.rfind(" (") {
        Some(i) if rest.ends_with(')') => (&rest[..i], &rest[i + 2..rest.len() - 1]),
        _ => (rest, ""),
    };
    let name = name.trim();
    (!name.is_empty()).then(|| PluginRef {
        name: name.into(),
        vendor: vendor.trim().into(),
    })
}

pub fn parse_rpp(text: &str) -> Vec<PluginRef> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        let Some(rest) = [
            "<VST ", "<VSTi ", "<AU ", "<AUi ", "<CLAP ", "<CLAPi ", "<LV2 ", "<LV2i ",
        ]
        .iter()
        .find_map(|p| l.strip_prefix(p)) else {
            continue;
        };
        let Some(start) = rest.find('"') else {
            continue;
        };
        let Some(end) = rest[start + 1..].find('"') else {
            continue;
        };
        if let Some(r) = split_ref(&rest[start + 1..start + 1 + end])
            && !out.contains(&r)
        {
            out.push(r);
        }
    }
    out
}

/// `<Tag Value="..."/>` value of the first occurrence of `tag` in `text`.
fn value_after<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let needle = format!("<{tag} Value=\"");
    let i = text.find(&needle)?;
    let rest = &text[i + needle.len()..];
    rest.split_once('"').map(|(v, _)| v)
}

/// Removes every `<Preset>...</Preset>` subrange from `block`. Real Live plugin
/// blocks carry a multi-KB Preset subtree between the scalar props and the
/// real `<Name>`/`<Manufacturer>` tags, and that subtree may itself contain a
/// decoy `<Name Value="" />` (or even a non-empty one) that must not be
/// mistaken for the block's own name. If a `<Preset>` has no closing tag, the
/// rest of the block is dropped (nothing trustworthy follows unterminated
/// markup). All offsets come from `find`, so they always land on char
/// boundaries — no fixed-byte windows.
fn excise_presets(block: &str) -> String {
    let mut out = String::with_capacity(block.len());
    let mut rest = block;
    loop {
        match rest.find("<Preset>") {
            None => {
                out.push_str(rest);
                return out;
            }
            Some(p) => {
                out.push_str(&rest[..p]);
                match rest[p..].find("</Preset>") {
                    Some(q) => rest = &rest[p + q + "</Preset>".len()..],
                    None => return out, // unterminated Preset: drop the rest
                }
            }
        }
    }
}

/// Extracts the plugin name/vendor from one opener..closer block, after
/// excising any `<Preset>` subtree so its contents can't shadow the block's
/// own `<PlugName>`/`<Name>`/`<Manufacturer>`.
fn ref_from_block(block: &str) -> Option<PluginRef> {
    let text = excise_presets(block);
    let name = value_after(&text, "PlugName").or_else(|| value_after(&text, "Name"))?;
    if name.is_empty() {
        return None;
    }
    let vendor = value_after(&text, "Manufacturer").unwrap_or("");
    Some(PluginRef {
        name: name.into(),
        vendor: vendor.into(),
    })
}

pub fn parse_als(bytes: &[u8]) -> Vec<PluginRef> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut xml = String::new();
    if GzDecoder::new(bytes).read_to_string(&mut xml).is_err() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for opener in [
        "<VstPluginInfo",
        "<Vst3PluginInfo",
        "<AuPluginInfo",
        "<ClapPluginInfo",
    ] {
        // These elements don't self-nest, so the first closer after the
        // opener is always this block's own — take the whole substring as
        // the block and resume scanning from just past it.
        let closer = format!("</{}>", &opener[1..]);
        let mut from = 0;
        while let Some(i) = xml[from..].find(opener) {
            let open_at = from + i;
            let body_at = open_at + opener.len();
            let Some(j) = xml[body_at..].find(&closer) else {
                break; // no closing tag for this opener kind past here
            };
            let close_end = body_at + j + closer.len();
            if let Some(r) = ref_from_block(&xml[open_at..close_end])
                && !out.contains(&r)
            {
                out.push(r);
            }
            from = close_end;
        }
    }
    out
}

/// Attribute value of `key="..."` inside one element span.
fn attr<'a>(el: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("{key}=\"");
    let i = el.find(&needle)?;
    el[i + needle.len()..].split_once('"').map(|(v, _)| v)
}

/// Studio One `.song`: a zip of XML documents. A plugin ref is any element
/// carrying `name="..."` plus a uid/deviceUID/manufacturer/vendor attribute —
/// deliberately greedy; refs that match no installed plugin are inert
/// downstream, missing a real device is the only failure that matters.
pub fn parse_song(bytes: &[u8]) -> Vec<PluginRef> {
    let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let Ok(mut file) = archive.by_index(i) else {
            continue;
        };
        if !file.name().to_lowercase().ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        use std::io::Read;
        if file.read_to_string(&mut xml).is_err() {
            continue;
        }
        // Walk element spans: substrings between '<' and the next '>'.
        let mut rest = xml.as_str();
        while let Some(open) = rest.find('<') {
            let Some(close) = rest[open..].find('>') else {
                break;
            };
            let el = &rest[open + 1..open + close];
            rest = &rest[open + close + 1..];
            let Some(name) = attr(el, "name") else {
                continue;
            };
            let vendor = attr(el, "manufacturer").or_else(|| attr(el, "vendor"));
            let has_uid = attr(el, "uid").is_some() || attr(el, "deviceUID").is_some();
            if name.is_empty() || (vendor.is_none() && !has_uid) {
                continue;
            }
            let r = PluginRef {
                name: name.trim().into(),
                vendor: vendor.unwrap_or("").trim().into(),
            };
            if !out.contains(&r) {
                out.push(r);
            }
        }
    }
    out
}

fn wanted(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    let lower = s.to_lowercase();
    (lower.ends_with(".rpp")
        || lower.ends_with(".als")
        || lower.ends_with(".song")
        || lower.ends_with(".logicx"))
        && !lower.ends_with(".rpp-bak")
        && !s.split('/').any(|seg| seg == "Backup")
}

pub fn find_projects(finder: &dyn ProjectFinder) -> Vec<PathBuf> {
    let mdfind = finder
        .mdfind(
            "kMDItemFSName == '*.rpp'c || kMDItemFSName == '*.als'c || \
             kMDItemFSName == '*.song'c || kMDItemFSName == '*.logicx'c",
        )
        .unwrap_or_default();
    let all = if mdfind.is_empty() {
        finder.walk_fallback()
    } else {
        mdfind
    };
    all.into_iter().filter(|p| wanted(p)).collect()
}

pub struct RealFinder;

impl ProjectFinder for RealFinder {
    fn mdfind(&self, query: &str) -> Result<Vec<PathBuf>, String> {
        let out = std::process::Command::new("mdfind")
            .arg(query)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("mdfind exited {}", out.status));
        }
        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(PathBuf::from)
            .collect())
    }

    fn walk_fallback(&self) -> Vec<PathBuf> {
        fn walk(dir: &std::path::Path, depth: usize, out: &mut Vec<PathBuf>) {
            if depth == 0 {
                return;
            }
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.to_string_lossy().to_lowercase().ends_with(".logicx") {
                        out.push(p);
                    } else {
                        walk(&p, depth - 1, out);
                    }
                } else {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            for sub in ["Documents", "Music"] {
                walk(&home.join(sub), 8, &mut out);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    const RPP: &str = r#"
  <TRACK
    <FXCHAIN
      <AU "AU: Ozone 12 Unlimiter (iZotope)" "iZotope: Ozone 12 Unlimiter" "" 1635083896 1514296652
      >
      <VST "VST3: Ozone 12 Vintage Limiter (iZotope)" "Ozone 12 Vintage Limiter.vst3" 0 "" 299732006{56}
      >
      <VST "VSTi: Serum (Xfer Records)" Serum.vst3 0 "" 1234{56}
      >
      <JS "utility/volume" ""
      >
      <CLAP "CLAP: Surge XT (Surge Synth Team)" surge-xt.clap 0 ""
      >
    >
  >
"#;

    #[test]
    fn rpp_extracts_name_vendor_and_skips_js() {
        let refs = parse_rpp(RPP);
        assert_eq!(refs.len(), 4);
        assert!(refs.contains(&PluginRef {
            name: "Ozone 12 Unlimiter".into(),
            vendor: "iZotope".into()
        }));
        assert!(refs.contains(&PluginRef {
            name: "Serum".into(),
            vendor: "Xfer Records".into()
        }));
        assert!(refs.contains(&PluginRef {
            name: "Surge XT".into(),
            vendor: "Surge Synth Team".into()
        }));
        assert!(refs.iter().all(|r| !r.name.contains("volume")));
    }

    #[test]
    fn rpp_extracts_lv2i_instrument() {
        let rpp = r#"
  <TRACK
    <FXCHAIN
      <LV2i "LV2i: Vitalium (Matt Tytel)" "vitalium#Vitalium" 0 "" 1{56}
      >
    >
  >
"#;
        let refs = parse_rpp(rpp);
        assert_eq!(refs.len(), 1);
        assert!(refs.contains(&PluginRef {
            name: "Vitalium".into(),
            vendor: "Matt Tytel".into()
        }));
    }

    fn gz(xml: &str) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(xml.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    const ALS: &str = r#"<?xml version="1.0"?><Ableton>
      <VstPluginInfo Id="1"><PlugName Value="TAL Reverb 4 Plugin" /></VstPluginInfo>
      <AuPluginInfo Id="2"><Name Value="Sumu" /><Manufacturer Value="Madrona Labs" /></AuPluginInfo>
      <Vst3PluginInfo Id="3"><Name Value="Pro-Q 3" /></Vst3PluginInfo>
      <SomethingElse><Name Value="Not A Plugin" /></SomethingElse>
    </Ableton>"#;

    #[test]
    fn als_extracts_plugin_blocks_only() {
        let refs = parse_als(&gz(ALS));
        assert_eq!(refs.len(), 3);
        assert!(refs.contains(&PluginRef {
            name: "TAL Reverb 4 Plugin".into(),
            vendor: "".into()
        }));
        assert!(refs.contains(&PluginRef {
            name: "Sumu".into(),
            vendor: "Madrona Labs".into()
        }));
        assert!(refs.contains(&PluginRef {
            name: "Pro-Q 3".into(),
            vendor: "".into()
        }));
    }

    #[test]
    fn als_corrupt_gzip_yields_empty() {
        assert!(parse_als(b"not gzip at all").is_empty());
    }

    // Real Live blocks: opener, scalar props, a multi-KB <Preset> subtree that
    // may itself carry a decoy (or non-empty) <Name>, and only then the real
    // <Name Value="..."/> — thousands of chars after the opener. Modeled on
    // `/tmp/real.xml` extracted from an actual .als.
    fn real_shaped_block(preset_filler: &str) -> String {
        format!(
            r#"<Vst3PluginInfo Id="0">
              <WinPosX Value="373" />
              <WinPosY Value="175" />
              <NumAudioInputs Value="1" />
              <NumAudioOutputs Value="1" />
              <IsPlaceholderDevice Value="false" />
              <Preset>
                <Vst3Preset Id="3">
                  <Name Value="Decoy" />
                  {preset_filler}
                </Vst3Preset>
              </Preset>
              <Name Value="Invigorate" />
              <Uid>
                <Fields.0 Value="1448301673" />
              </Uid>
            </Vst3PluginInfo>"#
        )
    }

    #[test]
    fn als_skips_preset_decoy_and_finds_real_name_far_after_opener() {
        let filler = "x".repeat(200);
        let xml = format!(
            r#"<?xml version="1.0"?><Ableton>{}</Ableton>"#,
            real_shaped_block(&filler)
        );
        let refs = parse_als(&gz(&xml));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "Invigorate");
        assert!(!refs.iter().any(|r| r.name == "Decoy"));
    }

    #[test]
    fn als_handles_multibyte_filler_in_preset_without_panicking() {
        let filler = "Größe müßig 音源 ".repeat(20);
        let xml = format!(
            r#"<?xml version="1.0"?><Ableton>{}</Ableton>"#,
            real_shaped_block(&filler)
        );
        let refs = parse_als(&gz(&xml));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "Invigorate");
    }

    #[test]
    #[ignore = "machine-local file: run explicitly with `cargo test real_als_smoke -- --ignored`"]
    fn real_als_smoke() {
        let path = "/Users/nikitagriaznov/Documents/REAPER Media/FS3/Untitled Project/Untitled.als";
        let bytes = std::fs::read(path).expect("real .als fixture must exist for this smoke test");
        let refs = parse_als(&bytes);
        assert!(
            !refs.is_empty(),
            "expected at least one plugin ref from the real project"
        );
        assert!(
            refs.iter().any(|r| r.name == "Invigorate"),
            "expected to find the real plugin name past its Preset blob, got {refs:?}"
        );
    }

    fn song_zip(xml: &str) -> Vec<u8> {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("Song/song.xml", opts).unwrap();
        z.write_all(xml.as_bytes()).unwrap();
        z.start_file("Song/mediapool.xml", opts).unwrap();
        z.write_all(b"<MediaPool></MediaPool>").unwrap();
        z.finish().unwrap().into_inner()
    }

    #[test]
    fn song_extracts_devices_with_uid_or_manufacturer() {
        let xml = r#"<?xml version="1.0"?>
          <Song>
            <Attributes name="Pro-Q 3" uid="{ABC-123}" manufacturer="FabFilter"/>
            <Attributes name="Serum" deviceUID="com.xfer.serum"/>
            <Attributes name="Untitled Track"/>
            <Marker name="Chorus" uid="not-a-plugin-but-harmless"/>
          </Song>"#;
        let refs = parse_song(&song_zip(xml));
        assert!(refs.contains(&PluginRef {
            name: "Pro-Q 3".into(),
            vendor: "FabFilter".into()
        }));
        assert!(refs.contains(&PluginRef {
            name: "Serum".into(),
            vendor: "".into()
        }));
        // name without uid/vendor/manufacturer is NOT extracted
        assert!(!refs.iter().any(|r| r.name == "Untitled Track"));
    }

    #[test]
    fn song_not_a_zip_yields_empty() {
        assert!(parse_song(b"definitely not a zip").is_empty());
    }

    #[test]
    fn song_dedupes_repeated_devices() {
        let xml = r#"<Song>
          <Attributes name="Serum" uid="a"/><Attributes name="Serum" uid="a"/>
        </Song>"#;
        assert_eq!(parse_song(&song_zip(xml)).len(), 1);
    }

    struct SpyFinder {
        mdfind_result: Result<Vec<PathBuf>, String>,
        walked: std::cell::Cell<bool>,
    }
    impl ProjectFinder for SpyFinder {
        fn mdfind(&self, _q: &str) -> Result<Vec<PathBuf>, String> {
            self.mdfind_result.clone()
        }
        fn walk_fallback(&self) -> Vec<PathBuf> {
            self.walked.set(true);
            vec![PathBuf::from("/walked/x.rpp")]
        }
    }

    #[test]
    fn discovery_prefers_mdfind_and_filters_noise() {
        let spy = SpyFinder {
            mdfind_result: Ok(vec![
                PathBuf::from("/p/Song.RPP"),
                PathBuf::from("/p/Backup/Song [1].als"),
                PathBuf::from("/p/Song.rpp-bak"),
                PathBuf::from("/p/Live Set.als"),
            ]),
            walked: std::cell::Cell::new(false),
        };
        let found = find_projects(&spy);
        assert_eq!(
            found,
            vec![
                PathBuf::from("/p/Song.RPP"),
                PathBuf::from("/p/Live Set.als")
            ]
        );
        assert!(!spy.walked.get());
    }

    #[test]
    fn discovery_falls_back_to_walk_when_mdfind_fails() {
        let spy = SpyFinder {
            mdfind_result: Err("no spotlight".into()),
            walked: std::cell::Cell::new(false),
        };
        let found = find_projects(&spy);
        assert_eq!(found, vec![PathBuf::from("/walked/x.rpp")]);
        assert!(spy.walked.get());
    }
}
