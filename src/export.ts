import type { Plugin } from "./types";

const HEADER = [
  "product", "name", "vendor", "version", "format", "scope", "category",
  "sizeBytes", "path", "bundleId", "packageId",
] as const;

// RFC 4180: quote when the field contains a comma, quote, or newline.
const cell = (v: string | number | null): string => {
  const s = v === null ? "" : String(v);
  return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};

export function exportCsv(plugins: Plugin[]): string {
  const rows = plugins.flatMap((p) =>
    p.installs.map((b) =>
      [p.name, b.name, b.vendor, b.version, b.format, b.scope, b.category ?? "",
       b.sizeBytes, b.path, b.bundleId, b.packageId ?? ""].map(cell).join(","),
    ),
  );
  return [HEADER.join(","), ...rows].join("\n") + "\n";
}

export function exportJson(plugins: Plugin[]): string {
  const products = plugins.map((p) => ({
    name: p.name,
    vendor: p.vendor,
    version: p.version,
    sizeBytes: p.sizeBytes,
    category: p.category,
    installs: p.installs.map((b) => ({
      name: b.name,
      vendor: b.vendor,
      version: b.version,
      format: b.format,
      scope: b.scope,
      category: b.category,
      sizeBytes: b.sizeBytes,
      path: b.path,
      bundleId: b.bundleId,
      packageId: b.packageId,
    })),
  }));
  return JSON.stringify(products, null, 2) + "\n";
}
