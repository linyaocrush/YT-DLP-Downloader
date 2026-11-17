<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";

  import { onDownloadUpdate, onQueueUpdate } from "./lib/api";
  import { downloadStore } from "./lib/download.svelte";
  import { settingsPage } from "./lib/settingsPage.svelte";
  import { settingsStore, ui } from "./lib/store.svelte";
  import DownloadView from "./views/DownloadView.svelte";
  import SettingsView from "./views/SettingsView.svelte";

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unsubscribeSettings: (() => void) | undefined;
    let unlistenQueue: (() => void) | undefined;
    let unlistenClose: (() => void) | undefined;

    void (async () => {
      unlistenClose = await getCurrentWindow().onCloseRequested(async event => {
        event.preventDefault();
        if (downloadStore.isBusy && !window.confirm("仍有任务运行。退出将停止当前任务并保留队列，下次启动可继续。确定退出？")) return;
        if (downloadStore.isBusy) downloadStore.cancel();
        try {
          await settingsStore.flush();
          if (settingsStore.error) throw {message: settingsStore.error};
          if (!downloadStore.isBusy) await downloadStore.persistQueue();
          await invoke("prepare_exit");
        } catch (error) { downloadStore.statusText = `退出失败：${(error as {message?: string})?.message ?? String(error)}`; }
      });
      unlisten = await onDownloadUpdate((update) => downloadStore.handleUpdate(update));
      unlistenQueue = await onQueueUpdate(update => downloadStore.handleQueueUpdate(update));
      await settingsStore.load();
      ui.initTheme();
      await Promise.all([downloadStore.initialize(), settingsPage.initialize()]);

      // Mirrors the WPF `ISettingsService.Changed` subscriptions.
      unsubscribeSettings = settingsStore.onChange((keys) => {
        ui.syncTheme();
        downloadStore.onSettingsChanged(keys);
      });
    })().catch(error => { downloadStore.statusText = `初始化失败：${error?.message ?? String(error)}`; });

    return () => {
      unlisten?.();
      unlistenQueue?.();
      unlistenClose?.();
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

{#if settingsStore.error}
  <div role="alert" style="padding:10px 18px;color:var(--env-missing)">{settingsStore.error}</div>
{/if}
{#if settingsStore.warning}<div role="alert" style="padding:10px 18px;color:var(--env-partial)">{settingsStore.warning}<button class="btn" onclick={() => settingsStore.warning = ""}>关闭提示</button></div>{/if}

{#if ui.tab === "download"}
  <DownloadView />
{:else}
  <SettingsView />
{/if}
