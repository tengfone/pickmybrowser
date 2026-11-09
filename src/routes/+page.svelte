<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import Picker from "$lib/Picker.svelte";
  import Settings from "$lib/Settings.svelte";
  import { api, applyOs, applyTheme } from "$lib/api";
  import { loadIcons } from "$lib/icons.svelte";

  const label = getCurrentWebviewWindow().label;

  // E2E builds only (`npm run build:wdio`): WebdriverIO's execute/mock bridge.
  if (import.meta.env.VITE_WDIO === "1") void import("@wdio/tauri-plugin");

  onMount(async () => {
    loadIcons();
    const s = await api.snapshot();
    applyOs(s.os);
    applyTheme(s.settings.theme);
  });
</script>

{#if label === "picker"}
  <Picker />
{:else}
  <Settings />
{/if}
