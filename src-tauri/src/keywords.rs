//! Curated keywords appended to each search document before embedding, so
//! function-word queries ("reverb", "equalizer") can match plugins whose
//! names are brand words. Static data, additive matching, no IO.

/// Strip non-alphanumerics + lowercase (mirrors the frontend grouping fold).
fn fold(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Folded-vendor substring → keywords. Keys are distinctive substrings so
/// vendor spelling variants match ("d16group" hits "d16groupaudiosoftware").
const VENDOR_KEYWORDS: &[(&str, &str)] = &[
    ("arturia", "analog synthesizer vintage keys emulation"),
    ("valhalla", "reverb delay echo"),
    ("fabfilter", "equalizer compressor limiter filter mixing"),
    ("talsoftware", "analog synthesizer effect"),
    ("d16group", "drum machine"),
    ("xfer", "wavetable synthesizer"),
    ("nativeinstruments", "sampler synthesizer drums"),
    ("izotope", "mastering mixing equalizer restoration"),
    ("ssl", "channel strip console equalizer compressor mixing"),
    (
        "solidstatelogic",
        "channel strip console equalizer compressor mixing",
    ),
    ("moog", "analog synthesizer bass ladder filter"),
    ("korg", "synthesizer keys"),
    ("roland", "synthesizer drum machine vintage"),
    (
        "noiseengineering",
        "modular oscillator synthesizer percussion",
    ),
    ("discodsp", "synthesizer sampler"),
    ("appliedacoustics", "physical modeling synthesizer"),
    ("tracktion", "synthesizer"),
    ("madronalabs", "synthesizer additive modular"),
    ("vcvrack", "modular synthesizer eurorack"),
    ("newfangledaudio", "synthesizer saturation limiter"),
    ("eventide", "harmonizer pitch delay reverb"),
    ("harrisonaudio", "channel strip console equalizer mixing"),
    ("unfilteredaudio", "delay glitch effect"),
    ("uhe", "analog wavetable synthesizer"),
    ("soundtoys", "saturation delay effect"),
    ("spectrasonics", "synthesizer sampler"),
    ("sonnox", "equalizer compressor mastering"),
    ("apple", "spatial audio renderer system"),
];

/// Folded-name substring → keywords. Keys must be >= 4 chars.
const NAME_KEYWORDS: &[(&str, &str)] = &[
    ("proq", "equalizer"),
    ("prol", "limiter"),
    ("proc", "compressor"),
    ("pror", "reverb"),
    ("vintageverb", "reverb"),
    ("supermassive", "reverb"),
    ("drumazon", "drum machine 909"),
    ("nepheton", "drum machine 808"),
    ("decimort", "bit crusher sampler"),
    ("kontakt", "sampler"),
    ("battery", "drums sampler"),
    ("serum", "wavetable synthesizer"),
    ("retromulator", "vintage sampler"),
    ("mariana", "analog bass synthesizer"),
];

pub fn enrich(name: &str, vendor: &str) -> String {
    let (fname, fvendor) = (fold(name), fold(vendor));
    let mut out: Vec<&str> = Vec::new();
    let hits = VENDOR_KEYWORDS
        .iter()
        .filter(|(k, _)| fvendor.contains(k))
        .chain(NAME_KEYWORDS.iter().filter(|(k, _)| fname.contains(k)));
    for (_, words) in hits {
        for w in words.split_whitespace() {
            if !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_substring_matches_spelling_variants() {
        assert!(enrich("Drumazon 2", "D16 Group Audio Software").contains("drum machine"));
        assert!(enrich("Drumazon 2", "d16group").contains("drum machine"));
    }

    #[test]
    fn vendor_and_name_matches_are_additive_and_deduped() {
        let k = enrich("Pro-Q 3", "FabFilter");
        // vendor gives "equalizer compressor limiter filter mixing", name gives "equalizer"
        assert!(k.contains("equalizer") && k.contains("compressor"));
        assert_eq!(
            k.matches("equalizer").count(),
            1,
            "keywords must be deduped"
        );
    }

    #[test]
    fn unknown_vendor_and_name_yield_empty() {
        assert_eq!(enrich("Obscuritron", "Nobody Knows Ltd"), "");
    }

    #[test]
    fn short_name_keys_are_absent() {
        assert!(NAME_KEYWORDS.iter().all(|(k, _)| k.len() >= 4));
    }
}
