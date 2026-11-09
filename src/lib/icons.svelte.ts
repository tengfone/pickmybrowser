import { invoke } from "@tauri-apps/api/core";

/** browser id → data: URL of the real OS icon (filled lazily, cached on disk by the backend). */
export const icons = $state<Record<string, string>>({});

let inflight: Promise<void> | null = null;
export function loadIcons(force = false) {
  if (inflight && !force) return inflight;
  inflight = invoke<Record<string, string>>("browser_icons")
    .then((m) => {
      for (const k of Object.keys(icons)) if (!(k in m)) delete icons[k];
      Object.assign(icons, m);
    })
    .catch(() => {});
  return inflight;
}
