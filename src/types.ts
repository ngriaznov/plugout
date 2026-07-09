export type Format = "AU" | "VST3" | "VST2" | "CLAP" | "AAX";
export type Scope = "user" | "system";
export type RemovalStatus = "trashed" | "failed";

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

export const FORMATS: Format[] = ["AU", "VST3", "VST2", "CLAP", "AAX"];

// One plugin product, i.e. all installed format bundles of it merged together.
export interface Plugin {
  key: string;
  name: string;
  vendor: string;
  version: string;
  installs: PluginBundle[];
  sizeBytes: number;
  scopes: Scope[];
}
