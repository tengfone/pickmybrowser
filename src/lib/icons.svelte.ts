import { invoke } from "@tauri-apps/api/core";

/** Browser id → data: URL of the real OS icon (filled lazily, cached on disk by the backend). */
export const icons = $state<Record<string, string>>({});

let request: Promise<void> | null = null;

/** Fetches icons once; pass `force` after a rescan to refresh them. Missing icons fall back to glyphs. */
export function loadIcons(force = false) {
  if (request && !force) return request;
  request = invoke<Record<string, string>>("browser_icons")
    .then((fresh) => {
      for (const id of Object.keys(icons)) {
        if (!(id in fresh)) delete icons[id];
      }
      Object.assign(icons, fresh);
    })
    .catch(() => {});
  return request;
}
