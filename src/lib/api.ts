import { invoke } from "@tauri-apps/api/core";

export type Engine = "chromium" | "firefox" | "safari" | "other";
export interface Profile {
  id: string;
  name: string;
}

export interface Browser {
  id: string;
  name: string;
  family: string;
  engine: Engine;
  exec: string;
  execArgs: string[];
  profiles: Profile[];
  enabled: boolean;
  /** The user's own picker key; unset means a position number. */
  hotkey?: string | null;
}

export interface Rule {
  pattern: string;
  browserId: string;
  profileId?: string | null;
  enabled: boolean;
}

export interface Recent {
  url: string;
  /** Unix time in seconds. */
  at: number;
  browserId?: string | null;
}

export interface Settings {
  theme: "system" | "dark" | "light";
  position: "cursor" | "center";
  storeRecents: boolean;
  recentLimit: number;
  stripTracking: boolean;
  warnLookalike: boolean;
  ruleCountdownMs: number;
  showTray: boolean;
  onboarded: boolean;
  autoUpdate: boolean;
  lastUpdateCheck: number;
}

export interface UpdateInfo {
  version: string;
  notes: string | null;
  /** This install can't update itself (Linux deb/rpm): link to `url` instead. */
  manual: boolean;
  url: string;
}

export interface Snapshot {
  browsers: Browser[];
  rules: Rule[];
  recents: Recent[];
  settings: Settings;
  isDefault: boolean;
  launchAtLogin: boolean;
  os: "windows" | "macos" | "linux";
  version: string;
}

/** A link waiting for the user to choose a browser. */
export interface Pending {
  url: string;
  original: string;
  /** Number of tracking parameters stripped from `original`. */
  removed: number;
  /** The well-known domain this URL appears to imitate, if any. */
  lookalike: string | null;
  rule: Omit<Rule, "enabled"> | null;
  countdownMs: number;
  /** Links waiting behind this one. */
  queued: number;
  /** Why the picker is showing instead of a rule's browser (it couldn't be started). */
  notice: string | null;
}

/** Thin typed wrappers over the Rust commands. */
export const api = {
  snapshot: () => invoke<Snapshot>("snapshot"),
  pending: () => invoke<Pending | null>("pending"),
  present: (width: number, height: number) => invoke("present_picker", { width, height }),
  resize: (width: number, height: number) => invoke("resize_picker", { width, height }),
  dismiss: () => invoke("dismiss"),
  open: (url: string, browserId: string, profileId: string | null, incognito: boolean) =>
    invoke("open_url", { url, browserId, profileId, private: incognito }),
  pick: (url: string) => invoke("pick_url", { url }),
  rescan: () => invoke<Browser[]>("rescan"),
  saveBrowsers: (browsers: Browser[]) => invoke("save_browsers", { browsers }),
  saveRules: (rules: Rule[]) => invoke("save_rules", { rules }),
  /** Index of the rule a URL would match, or null. */
  testRule: (rules: Rule[], url: string) => invoke<number | null>("test_rule", { rules, url }),
  saveSettings: (settings: Settings) => invoke("save_settings", { settings }),
  clearRecents: () => invoke("clear_recents"),
  setDefault: () => invoke<boolean>("set_default"),
  isDefault: () => invoke<boolean>("is_default"),
  setLaunchAtLogin: (on: boolean) => invoke("set_launch_at_login", { on }),
  resetAll: () => invoke<Browser[]>("reset_all"),
  uninstallApp: () => invoke<void>("uninstall_app"),
  openSettings: () => invoke("open_settings"),
  openExternal: (url: string) => invoke("open_external", { url }),
  updateStatus: () => invoke<UpdateInfo | null>("update_status"),
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  installUpdate: () => invoke<void>("install_update"),
};

/** Keys that pick a profile once a browser's profile list is expanded. */
export const PROFILE_KEYS = "qwertyuiop".split("");

/**
 * The picker key of every enabled browser, in list order. A browser's own key (e.g. "c" for
 * Chrome) wins; the others get the digits 1–9 in order, skipping digits someone chose.
 * Browsers left over (more than nine) get no key.
 */
export function pickerKeys(browsers: Browser[]): Map<string, string> {
  const enabled = browsers.filter((b) => b.enabled);
  const custom = new Set(enabled.map((b) => b.hotkey).filter((k): k is string => !!k));
  const digits = "123456789".split("").filter((d) => !custom.has(d));
  const keys = new Map<string, string>();
  for (const b of enabled) {
    const key = b.hotkey || digits.shift();
    if (key) keys.set(b.id, key);
  }
  return keys;
}

/** Split a URL for display: host (bold), middle (truncatable), tail (always visible). */
export function splitUrl(raw: string) {
  try {
    const url = new URL(raw);
    const host = url.host.replace(/^www\./, "");
    const rest = decodeURI(url.pathname + url.search + url.hash).replace(/^\/$/, "");
    const tail = rest.length > 24 ? rest.slice(-18) : "";
    const mid = rest.slice(0, rest.length - tail.length);
    return { secure: url.protocol === "https:", host, mid, tail, scheme: url.protocol };
  } catch {
    return { secure: false, host: raw, mid: "", tail: "", scheme: "" };
  }
}

export function applyTheme(theme: Settings["theme"]) {
  const systemDark = matchMedia("(prefers-color-scheme: dark)").matches;
  const dark = theme === "dark" || (theme === "system" && systemDark);
  const root = document.documentElement;
  root.classList.toggle("dark", dark);
  root.classList.toggle("light", !dark);
}

export function applyOs(os: string) {
  document.documentElement.dataset.os = os;
}

export const modKey = (os: string) => (os === "macos" ? "⌘" : "Ctrl");
