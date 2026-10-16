<script lang="ts">
  import { onMount } from "svelte";

  import { onDownloadUpdate } from "./lib/api";
  import { downloadStore } from "./lib/download.svelte";
  import { settingsPage } from "./lib/settingsPage.svelte";
  import { settingsStore, ui } from "./lib/store.svelte";
  import DownloadView from "./views/DownloadView.svelte";
  import SettingsView from "./views/SettingsView.svelte";

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unsubscribeSettings: (() => void) | undefined;

    void (async () => {
      unlisten = await onDownloadUpdate((update) => downloadStore.handleUpdate(update));
      await settingsStore.load();
      ui.initTheme();
      await Promise.all([downloadStore.initialize(), settingsPage.initialize()]);

      // Mirrors the WPF `ISettingsService.Changed` subscriptions.
      unsubscribeSettings = settingsStore.onChange(() => {
        ui.syncTheme();
        downloadStore.onSettingsChanged();
      });
    })();

    return () => {
      unlisten?.();
      unsubscribeSettings?.();
    };
  });
</script>

<div class="tabs">
  <button type="button" class="tab" class:on={ui.tab === "download"} onclick={() => (ui.tab = "download")}>
    下载
  </button>
  <button type="button" class="tab" class:on={ui.tab === "settings"} onclick={() => (ui.tab = "settings")}>
    设置
  </button>
</div>

{#if ui.tab === "download"}
  <DownloadView />
{:else}
  <SettingsView />
{/if}