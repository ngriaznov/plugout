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
            "<VST ", "<VSTi ", "<AU ", "<AUi ", "<CLAP ", "<CLAPi ", "<LV2 ",
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
        if let Some(r) = split_ref(&rest[start + 1..start + 1 + end]) {
            if !out.contains(&r) {
                out.push(r);
            }
        }
    }
    out
}

/// `<Tag Value="..."/>` value directly following `from` in `text`, if the tag
/// appears within `window` chars. The window is clamped to the nearest
/// closing tag so a compact sibling element's attributes (e.g. another
/// plugin block's `Manufacturer`) can't bleed into this one.
fn value_after<'a>(text: &'a str, from: usize, tag: &str, window: usize) -> Option<&'a str> {
    let limit = text[from..].find("</").map_or(window, |p| p.min(window));
    let hay = &text[from..text.len().min(from + limit)];
    let i = hay.find(&format!("<{tag} Value=\""))?;
    let rest = &hay[i + tag.len() + 9..];
    rest.split_once('"').map(|(v, _)| v)
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
        let mut from = 0;
        while let Some(i) = xml[from..].find(opener) {
            let at = from + i + opener.len();
            let name = value_after(&xml, at, "PlugName", 500)
                .or_else(|| value_after(&xml, at, "Name", 500));
            if let Some(name) = name.filter(|n| !n.is_empty()) {
                let vendor = value_after(&xml, at, "Manufacturer", 500).unwrap_or("");
                let r = PluginRef {
                    name: name.into(),
                    vendor: vendor.into(),
                };
                if !out.contains(&r) {
                    out.push(r);
                }
            }
            from = at;
        }
    }
    out
}

fn wanted(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy();
    let lower = s.to_lowercase();
    (lower.ends_with(".rpp") || lower.ends_with(".als"))
        && !lower.ends_with(".rpp-bak")
        && !s.split('/').any(|seg| seg == "Backup")
}

pub fn find_projects(finder: &dyn ProjectFinder) -> Vec<PathBuf> {
    let mdfind = finder
        .mdfind("kMDItemFSName == '*.rpp'c || kMDItemFSName == '*.als'c")
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
                    walk(&p, depth - 1, out);
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
    use flate2::{write::GzEncoder, Compression};
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
