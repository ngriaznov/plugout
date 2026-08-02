import { useRef, useState } from "react";
import type { Plugin, PluginBundle } from "./types";
import { applyRowSelection, type SelectionAnchor } from "./util";

export function useSelection(bundles: PluginBundle[], visible: PluginBundle[], rowOrder: Plugin[]) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  // Last row a checkbox action landed on — the origin for shift-click ranges.
  const anchor = useRef<SelectionAnchor | null>(null);

  const toggleInstall = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  // Inspecting a row (plain row click) anchors there without changing the
  // selection, Finder-style: click row A, then shift-click row F selects A…F.
  const anchorTo = (p: Plugin) => {
    anchor.current = { key: p.key, checked: true };
  };

  // Plain click toggles a row and anchors there; shift-click extends the
  // anchor's action across the visible range (see applyRowSelection).
  const togglePlugin = (p: Plugin, shift = false) => {
    const rows = rowOrder.map((pl) => ({ key: pl.key, ids: pl.installs.map((b) => b.id) }));
    const result = applyRowSelection(selected, rows, anchor.current, p.key, shift);
    anchor.current = result.anchor;
    setSelected(result.next);
  };

  const toggleAll = () =>
    setSelected((s) =>
      visible.every((b) => s.has(b.id))
        ? new Set([...s].filter((id) => !visible.some((b) => b.id === id)))
        : new Set([...s, ...visible.map((b) => b.id)]),
    );

  const clear = () => {
    anchor.current = null;
    setSelected(new Set());
  };

  const selectedBundles = bundles.filter((b) => selected.has(b.id));
  const selectedPluginCount = new Set(selectedBundles.map((b) => `${b.vendor} ${b.name}`)).size;
  const reclaimable = selectedBundles.reduce((n, b) => n + b.sizeBytes, 0);

  return {
    selected,
    setSelected,
    toggleInstall,
    togglePlugin,
    toggleAll,
    anchorTo,
    clear,
    selectedBundles,
    selectedPluginCount,
    reclaimable,
  };
}
