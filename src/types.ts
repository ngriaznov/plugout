export type Format = "AU" | "VST3" | "VST2" | "CLAP" | "AAX" | "APP";
export type Scope = "user" | "system";
export type Category = "instrument" | "effect" | "midiEffect";
export type RemovalStatus = "trashed" | "failed" | "canceled";

export const CATEGORY_LABELS: Record<Category, string> = {
  instrument: "Instrument",
  effect: "Effect",
  midiEffect: "MIDI Effect",
};

export interface PluginBundle {
  id: string;
  name: string;
  vendor: string;
  version: string;
  format: Format;
  bundleId: string;
  path: string;
  sizeBytes: number;
  scope: Scope;
  packageId: string | null;
  category: Category | null;
  copyright: string | null;
}

export interface PluginDetails {
  filesToTrash: string[];
  packageId: string | null;
}

export interface RemovalResult {
  id: string;
  path: string;
  status: RemovalStatus;
  message: string | null;
}

export const FORMATS: Format[] = ["AU", "VST3", "VST2", "CLAP", "AAX", "APP"];

// One plugin product, i.e. all installed format bundles of it merged together.
export interface Plugin {
  key: string;
  name: string;
  vendor: string;
  version: string;
  installs: PluginBundle[];
  sizeBytes: number;
  scopes: Scope[];
  category: Category | null;
  copyright: string | null;
}

export interface SupportFile {
  path: string;
  sizeBytes: number;
}

export interface RemovalPreview {
  supportFiles: SupportFile[];
  /** Packages whose files were kept because other installed plugins share them. */
  skippedShared: number;
}
