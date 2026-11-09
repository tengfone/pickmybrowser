import { invoke } from "@tauri-apps/api/core";

export type Engine = "chromium" | "firefox" | "safari" | "other";
export interface Profile { id: string; name: string }
export interface Browser {
  id: string; name: string; family: string; engine: Engine; exec: string;
  execArgs: string[]; profiles: Profile[]; enabled: boolean;
}
export interface Rule { pattern: string; browserId: string; profileId?: string | null; enabled: boolean }
export interface Recent { url: string; at: number; browserId?: string | null }
export interface Settings {
  theme: "system" | "dark" | "light"; position: "cursor" | "center";
  storeRecents: boolean; recentLimit: number; stripTracking: boolean; warnLookalike: boolean;
  ruleCountdownMs: number; showTray: boolean; onboarded: boolean;
}
export interface Snapshot {
  browsers: Browser[]; rules: Rule[]; recents: Recent[]; settings: Settings;
  isDefault: boolean; launchAtLogin: boolean; os: "windows" | "macos" | "linux"; version: string;
}
export interface Pending {
  url: string; original: string; removed: number; lookalike: string | null;
  rule: { pattern: string; browserId: string; profileId?: string | null } | null; countdownMs: number;
}

export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  pending: () => invoke<Pending | null>("pending"),
  present: (width: number, height: number) => invoke("present_picker", { width, height }),
  dismiss: () => invoke("dismiss"),
  open: (url: string, browserId: string, profileId: string | null, priv: boolean) =>
    invoke("open_url", { url, browserId, profileId, private: priv }),
  pick: (url: string) => invoke("pick_url", { url }),
  rescan: () => invoke<Browser[]>("rescan"),
  saveBrowsers: (browsers: Browser[]) => invoke("save_browsers", { browsers }),
  saveRules: (rules: Rule[]) => invoke("save_rules", { rules }),
  saveSettings: (settings: Settings) => invoke("save_settings", { settings }),
  clearRecents: () => invoke("clear_recents"),
  setDefault: () => invoke<boolean>("set_default"),
  isDefault: () => invoke<boolean>("is_default"),
  setLaunchAtLogin: (on: boolean) => invoke("set_launch_at_login", { on }),
  resetAll: () => invoke<Browser[]>("reset_all"),
  openSettings: () => invoke("open_settings"),
  openExternal: (url: string) => invoke("open_external", { url }),
};

export const PROFILE_KEYS = "qwertyuiop".split("");

/** Split a URL for display: host (bold), middle (truncatable), tail (kept). */
export function splitUrl(raw: string) {
  try {
    const u = new URL(raw);
    const host = u.host.replace(/^www\./, "");
    const rest = decodeURI(u.pathname + u.search + u.hash).replace(/^\/$/, "");
    const tail = rest.length > 24 ? rest.slice(-18) : "";
    const mid = tail ? rest.slice(0, rest.length - tail.length) : rest;
    return { secure: u.protocol === "https:", host, mid, tail, scheme: u.protocol };
  } catch {
    return { secure: false, host: raw, mid: "", tail: "", scheme: "" };
  }
}

export function applyTheme(theme: Settings["theme"]) {
  const dark = theme === "dark" || (theme === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.classList.toggle("light", !dark);
}

export function applyOs(os: string) {
  document.documentElement.dataset.os = os;
}

export const modKey = (os: string) => (os === "macos" ? "⌘" : "Ctrl");
