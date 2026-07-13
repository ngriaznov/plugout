import { useState } from "react";
import type { Plugin, PluginBundle } from "./types";

export function useSelection(bundles: PluginBundle[], visible: PluginBundle[]) {
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const toggleInstall = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const togglePlugin = (p: Plugin) =>
    setSelected((s) => {
      const n = new Set(s);
      const ids = p.installs.map((b) => b.id);
      if (ids.every((id) => n.has(id))) ids.forEach((id) => n.delete(id));
      else ids.forEach((id) => n.add(id));
      return n;
    });

  const toggleAll = () =>
    setSelected((s) =>
      visible.every((b) => s.has(b.id))
        ? new Set([...s].filter((id) => !visible.some((b) => b.id === id)))
        : new Set([...s, ...visible.map((b) => b.id)]),
    );

  const clear = () => setSelected(new Set());

  const selectedBundles = bundles.filter((b) => selected.has(b.id));
  const selectedPluginCount = new Set(selectedBundles.map((b) => `${b.vendor} ${b.name}`)).size;
  const reclaimable = selectedBundles.reduce((n, b) => n + b.sizeBytes, 0);

  return {
    selected,
    setSelected,
    toggleInstall,
    togglePlugin,
    toggleAll,
    clear,
    selectedBundles,
    selectedPluginCount,
    reclaimable,
  };
}
