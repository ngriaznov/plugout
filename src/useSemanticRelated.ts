import { useEffect, useMemo, useState } from "react";
import type { Format, Scope, Plugin, PluginBundle } from "./types";
import { semanticSearch, type SearchHit } from "./api";
import { gateHits, mergePlugins } from "./util";

export function useSemanticRelated(
  bundles: PluginBundle[],
  query: string,
  formatFilter: Format | "ALL",
  scopeFilter: Scope | "ALL",
): Plugin[] {
  const [relatedHits, setRelatedHits] = useState<SearchHit[]>([]);

  // Semantic hits trail the substring filter: debounce the query, embed it in
  // the backend, keep the sorted hit array. Backend failure ⇒ empty ⇒ the UI
  // silently stays substring-only. Gating happens later, in the memo, since
  // it depends on scope/query which can change independent of the fetch.
  useEffect(() => {
    const q = query.trim();
    if (q.length < 3) {
      setRelatedHits([]);
      return;
    }
    let cancelled = false;
    const t = window.setTimeout(() => {
      semanticSearch(q).then(
        (hits) => !cancelled && setRelatedHits(hits),
        () => !cancelled && setRelatedHits([]),
      );
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [query]);

  const related = useMemo(() => {
    if (relatedHits.length === 0 || query.trim().length < 3) return [];
    const ql = query.toLowerCase();
    const inScope = (b: PluginBundle) =>
      (formatFilter === "ALL" || b.format === formatFilter) &&
      (scopeFilter === "ALL" || b.scope === scopeFilter);
    const matchesText = (b: PluginBundle) => `${b.name} ${b.vendor}`.toLowerCase().includes(ql);
    const byId = new Map(bundles.map((b) => [b.id, b]));
    // Exclude substring/out-of-scope hits BEFORE gating, so the relative
    // floor is computed against the best hit that actually reads as related.
    const surviving = relatedHits.filter((h) => {
      const b = byId.get(h.id);
      return !!b && inScope(b) && !matchesText(b);
    });
    const gated = gateHits(surviving);
    if (gated.length === 0) return [];
    const semanticOnly = new Set(gated.map((h) => h.id));
    const score = new Map(gated.map((h) => [h.id, h.score]));
    // Merge semantic hits TOGETHER with the visible bundles so cross-format
    // identity bridges (shared bundle id) see every install of a product; a
    // product with any substring-matched install is already in the main list.
    const union = bundles.filter((b) => semanticOnly.has(b.id) || (inScope(b) && matchesText(b)));
    const merged = mergePlugins(union).filter((p) => p.installs.every((b) => semanticOnly.has(b.id)));
    const best = (p: Plugin) => Math.max(...p.installs.map((b) => score.get(b.id) ?? 0));
    return merged.sort((a, b) => best(b) - best(a));
  }, [bundles, relatedHits, query, formatFilter, scopeFilter]);

  return related;
}
