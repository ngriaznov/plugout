//! Semantic search over scanned plugins. The scan indexes one document per
//! bundle ("name vendor category"); queries embed once and rank by dot
//! product (vectors are L2-normalized, so dot == cosine).
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Deserialize)]
pub struct SearchDoc {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
}

/// Loose candidate floor, not a relevance judgment — calibrated by
/// measurement on a real library, where related-but-differently-named gear
/// scores as low as 0.25 while junk can reach 0.36. Final gating happens in
/// the frontend, relative to the best hit that survives substring exclusion.
pub const MIN_SCORE: f32 = 0.25;
/// Candidate cap: wide enough that the frontend's relative gate has room to
/// work with, not itself a relevance cutoff.
pub const TOP_K: usize = 16;

/// Replaced wholesale by each `index_search`; shared with `semantic_search`.
#[derive(Default, Clone)]
pub struct SearchIndex(pub Arc<Mutex<Vec<(String, Vec<f32>)>>>);

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn top_hits(index: &[(String, Vec<f32>)], query: &[f32]) -> Vec<SearchHit> {
    let mut hits: Vec<SearchHit> = index
        .iter()
        .map(|(id, v)| SearchHit { id: id.clone(), score: dot(v, query) })
        .filter(|h| h.score >= MIN_SCORE)
        .collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(TOP_K);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx(entries: &[(&str, [f32; 2])]) -> Vec<(String, Vec<f32>)> {
        entries.iter().map(|(id, v)| (id.to_string(), v.to_vec())).collect()
    }

    #[test]
    fn ranks_by_score_and_applies_threshold() {
        let index = idx(&[
            ("low", [0.1, 0.0]),
            ("borderline", [0.27, 0.0]),
            ("mid", [0.5, 0.0]),
            ("high", [0.9, 0.0]),
        ]);
        let hits = top_hits(&index, &[1.0, 0.0]);
        let ids: Vec<&str> = hits.iter().map(|h| h.id.as_str()).collect();
        // "low" (0.1) is under MIN_SCORE; "borderline" (0.27) now clears the
        // loosened floor even though it would have been cut at the old 0.30.
        assert_eq!(ids, vec!["high", "mid", "borderline"]);
    }

    #[test]
    fn caps_results_at_top_k() {
        let index: Vec<(String, Vec<f32>)> =
            (0..20).map(|i| (format!("p{i}"), vec![0.5, 0.0])).collect();
        assert_eq!(top_hits(&index, &[1.0, 0.0]).len(), TOP_K);
        assert_eq!(TOP_K, 16);
    }

    #[test]
    fn embeddings_rank_topically_related_names_higher() {
        let q = tern_engine::embed("reverb");
        let reverb = tern_engine::embed("TAL-Reverb-4 TAL Software effect");
        let synth = tern_engine::embed("Serum Xfer Records instrument");
        assert!(dot(&q, &reverb) > dot(&q, &synth));
    }
}
