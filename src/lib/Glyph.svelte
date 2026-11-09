<script lang="ts">
  // Shows the real OS icon when we have one; otherwise a drawn glyph for known families, or a monogram.
  import { icons } from "./icons.svelte";

  interface Props {
    family: string;
    name?: string;
    size?: number;
    id?: string;
  }
  let { family, name = "", size = 24, id = "" }: Props = $props();

  const src = $derived(id ? icons[id] : undefined);
  /** Stable per-family hue for the monogram fallback. */
  const hue = $derived([...family].reduce((acc, ch) => (acc * 31 + ch.charCodeAt(0)) % 360, 7));
  const letter = $derived((name || family).trim().charAt(0).toUpperCase() || "?");
</script>

{#if src}
<img class="ico" {src} width={size} height={size} alt="" draggable="false" />
{:else}
<svg class="ico" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  {#if family === "chrome"}
    <circle cx="12" cy="12" r="11" fill="#2E9E5B" /><path d="M12 1a11 11 0 0 1 9.5 5.5H12z" fill="#E0463A" /><path d="M21.5 6.5A11 11 0 0 1 12 23l4.8-8.3z" fill="#F2B632" /><circle cx="12" cy="12" r="5" fill="#fff" /><circle cx="12" cy="12" r="3.6" fill="#3B7BE8" />
  {:else if family === "firefox"}
    <circle cx="12" cy="12" r="11" fill="#5B2FB0" /><path d="M12 1a11 11 0 1 1-10.6 8C3 12 6 15 10 14c-3-1-3-5 0-6.5C9 5 10 3 12 1z" fill="#FF8A1F" /><circle cx="13" cy="12" r="4" fill="#5B2FB0" />
  {:else if family === "edge"}
    <circle cx="12" cy="12" r="11" fill="#1A7FD0" /><path d="M4 15a8 8 0 0 1 16-3c0 2-2 3-5 3H9a4 4 0 0 0 7 3 9 9 0 0 1-12-3z" fill="#4CD08A" />
  {:else if family === "safari"}
    <circle cx="12" cy="12" r="11" fill="#1E8FF0" /><circle cx="12" cy="12" r="8.5" fill="#fff" opacity=".95" /><path d="M16.5 7.5 13 13l-1-1z" fill="#E5402F" /><path d="M7.5 16.5 11 11l1 1z" fill="#9AA3AD" />
  {:else if family === "brave"}
    <circle cx="12" cy="12" r="11" fill="#F05A24" /><path d="M7 7h10l-1 8-4 3-4-3z" fill="#fff" /><circle cx="12" cy="11.5" r="2" fill="#F05A24" />
  {:else if family === "arc"}
    <circle cx="12" cy="12" r="11" fill="#E8456B" /><path d="M6 17A7 7 0 0 1 18 9" stroke="#fff" stroke-width="3" fill="none" stroke-linecap="round" /><circle cx="17" cy="16" r="2" fill="#FFD25A" />
  {:else if family === "vivaldi"}
    <circle cx="12" cy="12" r="11" fill="#E33A3A" /><path d="M7 8l5 9 5-9" stroke="#fff" stroke-width="2.6" fill="none" stroke-linecap="round" stroke-linejoin="round" />
  {:else if family === "opera"}
    <circle cx="12" cy="12" r="11" fill="#D51D2E" /><ellipse cx="12" cy="12" rx="4" ry="7" fill="#fff" /><ellipse cx="12" cy="12" rx="2.4" ry="5.4" fill="#D51D2E" />
  {:else if family === "tor"}
    <circle cx="12" cy="12" r="11" fill="#5E3A8C" /><circle cx="12" cy="12" r="7" fill="none" stroke="#fff" stroke-width="1.5" /><circle cx="12" cy="12" r="4" fill="none" stroke="#fff" stroke-width="1.5" /><circle cx="12" cy="12" r="1.5" fill="#fff" />
  {:else if family === "zen"}
    <circle cx="12" cy="12" r="11" fill="#2B2B2B" /><circle cx="12" cy="12" r="6.5" fill="none" stroke="#F4D35E" stroke-width="2.2" stroke-dasharray="34 8" />
  {:else if family === "librewolf"}
    <circle cx="12" cy="12" r="11" fill="#2F5DC9" /><circle cx="12" cy="12" r="6" fill="none" stroke="#9FD8FF" stroke-width="2.5" /><path d="M12 6v12" stroke="#fff" stroke-width="2" />
  {:else if family === "waterfox"}
    <circle cx="12" cy="12" r="11" fill="#0F6E9C" /><path d="M5 14c3-3 5 3 8 0s4 3 6 0v5H5z" fill="#7FD3F5" /><circle cx="12" cy="9" r="2.5" fill="#fff" />
  {:else if family === "chromium"}
    <circle cx="12" cy="12" r="11" fill="#4E86D8" /><circle cx="12" cy="12" r="5" fill="#fff" /><circle cx="12" cy="12" r="3.6" fill="#2C5FB0" />
  {:else}
    <circle cx="12" cy="12" r="11" fill="hsl({hue} 45% 38%)" />
    <text x="12" y="16.2" text-anchor="middle" font-size="12" font-weight="700" fill="#fff" font-family="IBM Plex Sans, system-ui">{letter}</text>
  {/if}
</svg>
{/if}

<style>
  .ico { flex: none; display: block; object-fit: contain; }
</style>
