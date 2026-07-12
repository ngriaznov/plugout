//! Real plugin metadata for the search corpus: VST3 bundles ship a
//! `moduleinfo.json` (JSON5 dialect — comments, trailing commas) whose
//! `Sub Categories` say what the plugin IS ("Fx", "EQ", "Dynamics"...).
//! Every failure returns no words — indexing must never break on a bundle.
use std::path::Path;

/// Subcategory token → embedding-friendly words. Unknown tokens pass through
/// lowercased; multi-token values like "Fx|Reverb" are split on '|' first.
const EXPANSIONS: &[(&str, &str)] = &[
    ("fx", "effect"),
    ("eq", "equalizer eq"),
    ("dynamics", "dynamics compressor"),
    ("dist", "distortion saturation"),
    ("distortion", "distortion saturation"),
    ("reverb", "reverb"),
    ("delay", "delay echo"),
    ("mastering", "mastering"),
    ("filter", "filter"),
    ("spatial", "spatial panner"),
    ("stereo", "stereo imaging width"),
    ("instrument", "instrument"),
    ("synth", "synthesizer"),
    ("sampler", "sampler"),
    ("drum", "drums"),
    ("pitch shift", "pitch"),
    ("modulation", "modulation chorus"),
    ("analyzer", "analyzer metering"),
];

pub fn subcategory_words(bundle_path: &Path) -> Vec<String> {
    [
        "Contents/Resources/moduleinfo.json",
        "Contents/moduleinfo.json",
    ]
    .iter()
    .map(|rel| bundle_path.join(rel))
    .find_map(|p| std::fs::read_to_string(p).ok())
    .map_or_else(Vec::new, |text| words_from_moduleinfo(&text))
}

/// Testable core: JSON5 text → deduped expanded words.
pub fn words_from_moduleinfo(text: &str) -> Vec<String> {
    let Ok(value) = json5::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    let classes = value.get("Classes").and_then(|c| c.as_array());
    for class in classes.into_iter().flatten() {
        let subs = class.get("Sub Categories").and_then(|s| s.as_array());
        for sub in subs.into_iter().flatten().filter_map(|s| s.as_str()) {
            for token in sub.split('|') {
                let token = token.trim().to_lowercase();
                let words = EXPANSIONS
                    .iter()
                    .find(|(k, _)| *k == token)
                    .map_or(token.clone(), |(_, w)| (*w).to_string());
                for w in words.split_whitespace() {
                    if !w.is_empty() && !out.iter().any(|o| o == w) {
                        out.push(w.to_string());
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real-world dialect: URL containing "//" inside a string (broke naive
    // comment stripping), a line comment, trailing commas.
    const MODULEINFO: &str = r#"{
  "Name": "SSL Fusion Violet EQ",
  "Factory Info": {
    "Vendor": "SSL",
    "URL": "https://www.solidstatelogic.com",
  },
  // module classes
  "Classes": [
    {
      "Name": "SSL Fusion Violet EQ",
      "Sub Categories": [ "Fx", "EQ", ],
    },
    {
      "Name": "SSL Fusion Violet EQ Controller",
      "Sub Categories": [ "Fx|Reverb" ],
    },
  ],
}"#;

    #[test]
    fn extracts_and_expands_subcategories() {
        let words = words_from_moduleinfo(MODULEINFO);
        // Fx -> effect, EQ -> equalizer eq, Fx|Reverb splits and dedupes
        assert_eq!(words, vec!["effect", "equalizer", "eq", "reverb"]);
    }

    #[test]
    fn unknown_tokens_pass_through_lowercased() {
        let words =
            words_from_moduleinfo(r#"{ "Classes": [ { "Sub Categories": ["Granular"] } ] }"#);
        assert_eq!(words, vec!["granular"]);
    }

    #[test]
    fn garbage_and_missing_yield_empty() {
        assert!(words_from_moduleinfo("not json at all {{{").is_empty());
        assert!(words_from_moduleinfo(r#"{ "Classes": [] }"#).is_empty());
        assert!(subcategory_words(Path::new("/nonexistent/x.vst3")).is_empty());
    }
}
