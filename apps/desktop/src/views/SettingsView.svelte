<script lang="ts">
  import Select from "../lib/components/Select.svelte";
  import Switch from "../lib/components/Switch.svelte";
  import Radio from "../lib/components/Radio.svelte";
  import { settingsStore } from "../lib/store.svelte";
  import { settingsPage } from "../lib/settingsPage.svelte";
  import type { AppTheme, ProxyProtocol } from "../lib/types";

  const themeOptions: { value: AppTheme; label: string }[] = [
    { value: "System", label: "跟随系统" },
    { value: "Light", label: "浅色" },
    { value: "Dark", label: "深色" },
  ];

  const protocolOptions: { value: ProxyProtocol; label: string }[] = [
    { value: "Http", label: "http" },
    { value: "Https", label: "https" },
    { value: "Socks5", label: "socks5" },
  ];

  const environmentClass = $derived(
    settingsPage.isEnvironmentReady
      ? "env-ready"
      : settingsPage.isEnvironmentPartial
        ? "env-partial"
        : "env-missing",
  );
</script>

<div class="page">
  <h1 class="page-title">设置</h1>

  <!-- 外观 -->
  <div class="card">
    <div class="grid-2">
      <div>
        <p class="card-title">外观</p>
        <p class="subtitle">选择浅色或深色主题，也可以跟随 Windows 系统设置自动切换。</p>
      </div>
      <Select
        value={settingsStore.value.Theme}
        options={themeOptions}
        width="140px"
        onchange={(value) => settingsPage.setTheme(value)}
      />
    </div>
  </div>

  <!-- 网络代理 -->
  <div class="card mt">
    <div class="grid-2">
      <div>
        <p class="card-title">网络代理</p>
        <p class="subtitle">
          开启后，下载与解析会按名单规则自动为 yt-dlp 添加 --proxy 参数。总开关关闭时永远不添加。
        </p>
      </div>
      <div style="padding-top:2px">
        <Switch
          checked={settingsStore.value.ProxyEnabled}
          title="代理总开关"
          onchange={(value) => settingsPage.setProxyEnabled(value)}
        />
      </div>
    </div>

    <div class="subpanel">
      <p class="panel-title">代理服务器</p>
      <div class="form-row">
        <span class="field-label">协议</span>
        <Select
          value={settingsStore.value.ProxyProtocol}
          options={protocolOptions}
          width="100px"
          onchange={(value) => settingsPage.setProxyProtocol(value)}
        />
        <span style="width:16px"></span>
        <span class="field-label">IP / 主机</span>
        <input
          type="text"
          class="grow"
          style="min-width:180px"
          placeholder="例如 127.0.0.1 或 proxy.example.com"
          value={settingsStore.value.ProxyHost}
          oninput={(event) => settingsPage.setProxyHost(event.currentTarget.value)}
        />
        <span style="width:16px"></span>
        <span class="field-label">端口</span>
        <input
          type="text"
          style="width:84px"
          placeholder="1-65535，例如 7890"
          value={settingsStore.value.ProxyPort}
          oninput={(event) => settingsPage.setProxyPort(event.currentTarget.value)}
        />
      </div>
    </div>

    <div class="subpanel">
      <p class="panel-title">名单规则</p>
      <p class="subtitle">选择黑名单或白名单模式，并维护对应的网站名单。</p>

      <div class="form-row" style="gap:20px">
        <span class="field-label">名单模式</span>
        <Radio
          checked={settingsStore.value.ProxyListMode === "Blacklist"}
          label="黑名单（名单内的网站不使用代理）"
          onchange={() => settingsPage.setProxyListMode("Blacklist")}
        />
        <Radio
          checked={settingsStore.value.ProxyListMode === "Whitelist"}
          label="白名单（仅名单内的网站使用代理）"
          onchange={() => settingsPage.setProxyListMode("Whitelist")}
        />
      </div>

      <div class="form-row">
        <input
          type="text"
          class="grow"
          placeholder="例如 https://www.youtube.com/"
          value={settingsPage.newProxySite}
          oninput={(event) => (settingsPage.newProxySite = event.currentTarget.value)}
          onkeydown={(event) => {
            if (event.key === "Enter") settingsPage.addProxySite();
          }}
        />
        <button
          type="button"
          class="btn"
          disabled={!settingsPage.newProxySite.trim()}
          onclick={() => settingsPage.addProxySite()}
        >
          添加网站
        </button>
      </div>

      {#if settingsStore.value.ProxySites.length > 0}
        <div style="margin-top:10px">
          {#each settingsStore.value.ProxySites as site (site)}
            <span class="tag">
              <span>{site}</span>
              <button
                type="button"
                class="tag-remove"
                title="移除该网站"
                onclick={() => settingsPage.removeProxySite(site)}
              >
                ✕
              </button>
            </span>
          {/each}
        </div>
      {/if}

      <p class="subtitle">{settingsPage.proxyStatus}</p>
    </div>
  </div>

  <!-- 环境检测 -->
  <div class="card mt">
    <div class="grid-2">
      <div>
        <p class="card-title">环境检测</p>
        <p class="subtitle">
          本工具依赖本机的 yt-dlp 与 ffmpeg，不会内置或自动下载。程序启动时会先从系统 PATH
          中自动查找，未找到时可在此手动指定。
        </p>
      </div>
      <div class="env-indicator" title={settingsPage.environmentStatusText} style="margin-top:2px">
        <span class="halo {environmentClass}"></span>
        <span class="core {environmentClass}"></span>
      </div>
    </div>

    <div class="subpanel">
      <p class="panel-title">yt-dlp</p>
      <p class="subtitle">用于解析视频信息与下载。</p>
      <div class="form-row">
        <input
          type="text"
          class="grow"
          placeholder="yt-dlp.exe 的完整路径，例如 D:\tools\yt-dlp.exe"
          value={settingsStore.value.YtDlpPath}
          oninput={(event) => void settingsPage.setYtDlpPath(event.currentTarget.value)}
          onchange={() => void settingsPage.refreshDetection()}
        />
        <button type="button" class="btn" onclick={() => settingsPage.browseYtDlp()}>浏览…</button>
        <button type="button" class="btn" onclick={() => settingsPage.rescan()}>重新检测</button>
      </div>

      {#if settingsPage.showYtDlpDownloadHint}
        <button type="button" class="btn-link" onclick={() => settingsPage.openYtDlpDownload()}>
          未找到 yt-dlp？点击前往官方下载页面
        </button>
      {/if}

      <div class="subpanel" style="background:var(--accent-soft); border:none; padding:12px 10px">
        <span>{settingsPage.statusText}</span>
      </div>
    </div>

    <div class="subpanel">
      <p class="panel-title">ffmpeg</p>
      <p class="subtitle">合成音视频、转换格式时需要 ffmpeg。</p>

      {#if settingsPage.showFfmpegManualControls}
        <div class="form-row">
          <input
            type="text"
            class="grow"
            placeholder="ffmpeg.exe 的完整路径，例如 D:\tools\ffmpeg.exe"
            value={settingsStore.value.FfmpegPath}
            oninput={(event) => settingsPage.setFfmpegPath(event.currentTarget.value)}
          />
          <button type="button" class="btn" onclick={() => settingsPage.browseFfmpeg()}>浏览…</button>
          <button
            type="button"
            class="btn"
            disabled={!settingsStore.value.FfmpegPath.trim()}
            onclick={() => settingsPage.clearFfmpeg()}
          >
            清除
          </button>
        </div>
      {/if}

      {#if settingsPage.showFfmpegDownloadHint}
        <button type="button" class="btn-link" onclick={() => settingsPage.openFfmpegDownload()}>
          未找到 ffmpeg？点击前往官方下载页面
        </button>
      {/if}

      <div class="subpanel" style="background:var(--accent-soft); border:none; padding:12px 10px">
        <span>{settingsPage.ffmpegStatus}</span>
      </div>
    </div>
  </div>

  <!-- Cookie 文件夹 -->
  <div class="card mt">
    <p class="card-title">Cookie 文件夹</p>
    <p class="subtitle">
      选择一个文件夹，程序会自动识别其中的所有 Cookie 文件，并在下载页以下拉列表的形式供你选择。未设置时下载页将不使用
      Cookie。
    </p>

    <div class="form-row">
      <input
        type="text"
        class="grow"
        title={settingsStore.value.CookieFolder}
        value={settingsStore.value.CookieFolder}
        oninput={(event) => settingsPage.setCookieFolder(event.currentTarget.value)}
      />
      <button type="button" class="btn" onclick={() => settingsPage.browseCookieFolder()}>浏览…</button>
      <button
        type="button"
        class="btn"
        disabled={!settingsStore.value.CookieFolder.trim()}
        onclick={() => settingsPage.clearCookieFolder()}
      >
        清除
      </button>
    </div>

    <p class="subtitle">{settingsPage.cookieFolderStatus}</p>
  </div>

  <!-- 关于 -->
  <div class="card mt">
    <p class="card-title">关于</p>
    <p class="about-paragraph">
      本工具是对本地 yt-dlp / ffmpeg 的图形化封装，不内置或下载任何外部程序。链接与音视频源均直接解析自你提供的
      URL。
    </p>
    <p class="about-paragraph">Copyright © 2026 凛ふわ狐</p>
  </div>
</div>