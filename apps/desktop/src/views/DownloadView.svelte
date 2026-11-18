<script lang="ts">
  import Chip from "../lib/components/Chip.svelte";
  import ProgressBar from "../lib/components/ProgressBar.svelte";
  import Radio from "../lib/components/Radio.svelte";
  import Segmented from "../lib/components/Segmented.svelte";
  import Select from "../lib/components/Select.svelte";
  import Switch from "../lib/components/Switch.svelte";
  import { settingsStore } from "../lib/store.svelte";
  import {
    downloadStore,
    itemSelectionInfo,
    itemStatusText,
  } from "../lib/download.svelte";
  import { RESOLUTION_CATALOG, type DownloadKind } from "../lib/types";

  const kindOptions: { value: DownloadKind; label: string }[] = [
    { value: "Default", label: "默认" },
    { value: "VideoOnly", label: "仅视频" },
    { value: "AudioOnly", label: "仅音频" },
  ];

  const cookieOptions = $derived([
    { value: "", label: "无（不使用 Cookie）" },
    ...downloadStore.cookieListing.files.map((file) => ({ value: file.path, label: file.name })),
  ]);

  const proxyColor = $derived(
    downloadStore.proxy.willApply ? "var(--proxy-on)" : "var(--proxy-off)",
  );

  let importDialog: HTMLDialogElement;
  let importInput: HTMLTextAreaElement;
  let rangeStart = $state("1");
  let rangeEnd = $state("1");

  function openImportDialog(): void {
    if (downloadStore.isBusy || importDialog.open) return;
    importDialog.showModal();
    importInput.focus();
  }

</script>

<div class="page" onfocusout={() => void settingsStore.flush()}>
  <!-- 下载设置 -->
  <div class="card">
    <!-- 视频链接 -->
    <div class="form-row" style="margin-top:0">
      <span class="field-label" style="width:96px">视频链接</span>
      <input
        type="text"
        class="grow"
        style="min-width:200px"
        disabled={downloadStore.isBusy}
        aria-label="视频链接"
        placeholder="输入视频链接；双击弹出批量输入链接窗口"
        value={downloadStore.url}
        ondblclick={openImportDialog}
        oninput={(event) => downloadStore.setUrl(event.currentTarget.value)}
      />
      <button
        type="button"
        class="btn-primary"
        disabled={!downloadStore.canParse}
        title="解析当前链接；播放列表会先显示条目选择界面；队列模式下按顺序解析队列中的视频"
        onclick={() => downloadStore.parse()}
      >
        解析
      </button>
      <button
        type="button"
        class="btn"
        disabled={!downloadStore.canAddToQueue}
        title="将当前链接加入下载队列，下载设置与单个视频相同"
        onclick={() => downloadStore.addToQueue()}
      >
        加入队列
      </button>
    </div>

    <!-- 保存到 -->
    <div class="form-row">
      <span class="field-label" style="width:96px">保存到</span>
      <input
        type="text"
        class="grow"
        style="min-width:200px"
        title={downloadStore.downloadDirectory}
        value={downloadStore.downloadDirectoryInput}
        oninput={(event) => downloadStore.setDownloadDirectory(event.currentTarget.value)}
      />
      <button type="button" class="btn" onclick={() => downloadStore.browseFolder()}>浏览…</button>
      <button type="button" class="btn" onclick={() => downloadStore.openDirectory()}>打开目录</button>
    </div>

    <!-- 文件名 -->
    <div class="form-row" style="align-items:flex-start">
      <span class="field-label" style="width:96px; padding-top:6px">文件名</span>
      <div class="grow" style="min-width:0">
        <div class="row" style="gap:20px">
          <Radio
            checked={downloadStore.isTemplateNameMode}
            label="模板模式"
            onchange={() => downloadStore.setNameMode(true)}
          />
          <Radio
            checked={downloadStore.isFixedNameMode}
            label="文件名模式"
            onchange={() => downloadStore.setNameMode(false)}
          />
        </div>

        {#if downloadStore.isTemplateNameMode}
          <input
            type="text"
            readonly
            class="mono"
            style="margin-top:8px; font-size:13px; background:var(--placeholder)"
            value={downloadStore.outputTemplate}
          />
          <div class="chip-list" style="margin-top:10px">
            {#each downloadStore.templateFields as field (field.token)}
              <Chip
                selected={field.selected}
                label={field.label}
                title={field.token}
                onchange={() => downloadStore.toggleTemplateField(field.token)}
              />
            {/each}
          </div>
        {:else}
          <input
            type="text"
            style="margin-top:8px"
            value={downloadStore.fixedFileName}
            oninput={(event) => downloadStore.setFixedFileName(event.currentTarget.value)}
          />
        {/if}

        <p class="hint">{downloadStore.outputNameHint}</p>
        <p class="hint">重名策略：跳过已存在文件，不覆盖；重试保留该任务首次使用的文件名与下载选项。</p>
        {#if downloadStore.isFixedNameMode && downloadStore.queue.length > 1}
          <p class="hint">批量文件名：{downloadStore.fixedFileName || "文件名"} - 001、002…（自动添加扩展名）</p>
        {/if}
      </div>
    </div>

    <!-- Cookie 文件 -->
    <div class="form-row">
      <span class="field-label" style="width:96px">Cookie 文件</span>
      <div class="grow" style="min-width:200px">
        <Select
          value={downloadStore.selectedCookiePath ?? ""}
          options={cookieOptions}
          disabled={!downloadStore.cookieListing.exists}
          onchange={(value) => downloadStore.setCookie(value ? value : null)}
        />
      </div>
      <button
        type="button"
        class="btn"
        title="重新扫描 Cookie 文件夹"
        onclick={() => downloadStore.refreshCookieListing()}
      >
        重新扫描
      </button>
    </div>

    <p
      class="hint"
      class:accent={downloadStore.cookieListing.needsSetup}
      style="margin-left:104px"
    >
      {downloadStore.cookieListing.hint}
    </p>

    <!-- 下载方式 -->
    <div class="form-row">
      <span class="field-label" style="width:96px">下载方式</span>
      <Radio
        checked={downloadStore.isAutoMode}
        label="自动选择"
        onchange={() => downloadStore.setManualMode(false)}
      />
      <span style="width:20px"></span>
      <Radio
        checked={downloadStore.isManualMode}
        label="手动选择"
        onchange={() => downloadStore.setManualMode(true)}
      />
      <span class="grow"></span>
      <span class="field-label" style="margin-right:8px">线程数</span>
      <input
        type="number"
        style="width:56px"
        min="1"
        max="64"
        title="同时下载的分片数（1-64），默认 4"
        value={downloadStore.threads}
        onchange={(event) => downloadStore.setThreads(Number(event.currentTarget.value))}
      />
      <button
        type="button"
        class="btn-primary"
        disabled={!downloadStore.canDownload}
        onclick={() => downloadStore.download()}
      >
        开始下载
      </button>
      <button
        type="button"
        class="btn"
        disabled={!downloadStore.canCancel}
        onclick={() => downloadStore.cancel()}
      >
        停止{downloadStore.isQueueMode ? "队列" : "任务"}
      </button>
      <span class="muted" style="margin-left:16px">{downloadStore.ytDlpStatus}</span>
    </div>

    <!-- 下载内容 -->
    <div class="form-row">
      <span class="field-label" style="width:96px">下载内容</span>
      <Segmented
        value={downloadStore.downloadKind}
        options={kindOptions}
        width="260px"
        onchange={(value) => downloadStore.setDownloadKind(value)}
      />
      {#if downloadStore.isAutoMode}
        <span class="field-label" style="margin-left:20px; margin-right:8px">分辨率</span>
        <Select
          value={downloadStore.resolution}
          options={RESOLUTION_CATALOG}
          width="132px"
          onchange={(value) => downloadStore.setResolution(value)}
        />
      {/if}
      <span class="grow"></span>
      <span class="proxy-pill" title={downloadStore.proxy.detail}>
        <span class="dot" style={`background:${proxyColor}`}></span>
        <span style={`color:${proxyColor}`}>{downloadStore.proxy.text}</span>
      </span>
    </div>

    <!-- 下载封面 -->
    <div class="form-row">
      <span class="field-label" style="width:96px">下载封面</span>
      <Switch
        checked={downloadStore.writeThumbnail}
        title="开启后，随视频额外下载一张封面图片（独立保存，不嵌入视频）"
        onchange={(value) => downloadStore.setWriteThumbnail(value)}
      />
      <span class="muted" style="margin-left:12px; font-size:12.5px">
        开启后额外保存封面图片；关闭则不下载。封面保存为独立图片文件，不会合成进视频。
      </span>
    </div>
  </div>

  <!-- 直播提示 -->
  {#if downloadStore.showStreamBanner}
    <div class="card mt">
      <div class="banner">
        <span
          class="dot"
          style={`width:10px; height:10px; margin-top:5px; background:${downloadStore.isLive
            ? "var(--live)"
            : "var(--upcoming)"}`}
        ></span>
        <div>
          <div
            class="banner-title"
            style={`color:${downloadStore.isLive ? "var(--live)" : "var(--upcoming)"}`}
          >
            {downloadStore.streamBannerTitle}
          </div>
          <div class="banner-detail">{downloadStore.streamBannerDetail}</div>
        </div>
      </div>
    </div>
  {/if}

  <!-- 解析结果 -->
  {#if downloadStore.showMediaCard}
    <div class="card mt">
      <div class="row" style="align-items:center">
        {#if downloadStore.thumbnailUrl}
          <img
            class="cover cover-160"
            src={downloadStore.thumbnailUrl}
            alt=""
            referrerpolicy="no-referrer"
          />
        {/if}
        <div style="margin-left:16px; min-width:0">
          <div style="font-size:17px; font-weight:600; line-height:1.4">
            {downloadStore.videoTitle}
          </div>
          <div class="muted" style="margin-top:6px">{downloadStore.metaText}</div>
        </div>
      </div>
    </div>
  {/if}

  <!-- 源列表（手动模式） -->
  {#if downloadStore.showSourceTables}
    <div style="display:flex; gap:12px; margin-top:12px; min-height:220px">
      {#if downloadStore.showVideoSources}
        <div class="card" style="flex:1; padding:12px; min-width:0; margin-top:0">
          <div class="semi" style="margin:0 0 8px 4px">视频源</div>
          <div class="table-scroll">
            <table class="data">
              <thead>
                <tr>
                  <th>格式 ID</th>
                  <th>分辨率</th>
                  <th>帧率</th>
                  <th>码率</th>
                  <th>视频编码</th>
                  <th>类型</th>
                  <th>容器</th>
                </tr>
              </thead>
              <tbody>
                {#each downloadStore.activeVideos as video (video.formatId)}
                  <tr
                    class:selected={downloadStore.activeSelectedVideo?.formatId === video.formatId}
                    onclick={() => (downloadStore.activeSelectedVideo = video)}
                  >
                    <td>{video.formatId}</td>
                    <td>{video.resolution}</td>
                    <td>{video.fpsText}</td>
                    <td>{video.bitrateText}</td>
                    <td>{video.codec}</td>
                    <td>{video.kind}</td>
                    <td>{video.container}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {/if}

      {#if downloadStore.showAudioSources}
        <div class="card" style="flex:1; padding:12px; min-width:0; margin-top:0">
          <div class="semi" style="margin:0 0 8px 4px">音频源</div>
          <div class="table-scroll">
            <table class="data">
              <thead>
                <tr>
                  <th>格式 ID</th>
                  <th>码率</th>
                  <th>音频编码</th>
                  <th>容器</th>
                </tr>
              </thead>
              <tbody>
                {#each downloadStore.activeAudios as audio (audio.formatId)}
                  <tr
                    class:selected={downloadStore.activeSelectedAudio?.formatId === audio.formatId}
                    onclick={() => (downloadStore.activeSelectedAudio = audio)}
                  >
                    <td>{audio.formatId}</td>
                    <td>{audio.bitrateText}</td>
                    <td>{audio.codec}</td>
                    <td>{audio.container}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <!-- 状态 + 选择提示 -->
  <div class="status-line">
    <div class="grow">{downloadStore.statusText}</div>
    {#if downloadStore.lastOutputPath}<button class="btn" onclick={() => downloadStore.reveal(downloadStore.lastOutputPath)}>定位文件</button>{/if}
    <div class="selection-hint">{downloadStore.selectionHint}</div>
  </div>

  <!-- 进度 / 队列 / 日志 -->
  <div style="margin-top:10px">
    {#if downloadStore.isProgressVisible}
      <div class="muted small" style="margin:0 0 6px 2px">{downloadStore.progressInfoText}</div>
      <div class="row">
        <div class="grow"><ProgressBar value={downloadStore.progress} /></div>
        <span class="muted" style="margin-left:10px">{downloadStore.progressText}</span>
      </div>
    {/if}

    {#if downloadStore.isQueueMode}
      <div class="card mt">
        <div class="row">
          <span class="semi" style="font-size:15px">下载队列</span>
          <span style="color:var(--accent); margin-left:12px">{downloadStore.queueProgressText}</span>
          <span class="grow"></span>
          <button class="btn" disabled={downloadStore.isBusy || !downloadStore.queue.some(item => item.state === "Failed")} onclick={() => downloadStore.retryItems(downloadStore.queue.filter(item => item.state === "Failed").map(item => item.id))}>重试失败任务</button>
          <button
            type="button"
            class="btn"
            disabled={!downloadStore.canClearQueue}
            onclick={() => downloadStore.clearQueue()}
          >
            清空队列
          </button>
        </div>

        <div class="list" style="margin-top:10px; max-height:340px">
          {#each downloadStore.queue as item (item.id)}
            <div
              class="queue-item"
              class:selected={downloadStore.selectedQueueItemId === item.id}
              role="button"
              tabindex="0"
              onclick={() => (downloadStore.selectedQueueItemId = item.id)}
              onkeydown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  downloadStore.selectedQueueItemId = item.id;
                }
              }}
            >
              {#if item.thumbnailUrl}
                <img
                  class="cover cover-112"
                  src={item.thumbnailUrl}
                  alt=""
                  referrerpolicy="no-referrer"
                />
              {/if}
              <div class="queue-body">
                <div class="queue-title" title={item.url}>{item.title}</div>
                <div class="muted ellipsis" style="font-size:12px; margin-top:3px">
                  {itemStatusText(item)}
                </div>
                {#if item.error}<details><summary>错误详情</summary><div style="user-select:text;overflow-wrap:anywhere">{item.error}</div></details>{/if}
                {#if item.selectedVideo || item.selectedAudio}
                  <div class="ellipsis" style="font-size:12px; margin-top:3px; color:var(--accent)">
                    {itemSelectionInfo(item)}
                  </div>
                {/if}
                {#if item.state === "Parsing" || item.state === "Downloading"}
                  <div style="margin-top:6px"><ProgressBar value={item.progress} /></div>
                {/if}
              </div>
              <button
                type="button"
                class="btn"
                style="padding:4px 10px"
                disabled={downloadStore.isBusy}
                onclick={(event) => {
                  event.stopPropagation();
                  downloadStore.removeQueueItem(item.id);
                }}
              >
                移除
              </button>
              {#if item.outputPath}
                <button class="btn" onclick={(event) => { event.stopPropagation(); void downloadStore.reveal(item.outputPath); }}>定位文件</button>
              {/if}
              {#if ["Failed", "Cancelled", "Completed"].includes(item.state)}
                <button class="btn" disabled={downloadStore.isBusy} onclick={(event) => {event.stopPropagation(); void downloadStore.retryItems([item.id]);}}>{item.state === "Completed" ? "重新下载" : "重试"}</button>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <details class="logs">
      <summary>日志</summary>
      <button class="btn" onclick={() => downloadStore.copyDiagnostics()}>复制诊断信息</button>
      <div class="log" style="margin-top:8px">
        {#each downloadStore.logs as line, index (index)}
          <div class="log-line">{line}</div>
        {/each}
      </div>
    </details>
  </div>
</div>

{#if downloadStore.playlistChoice}
  {@const playlist = downloadStore.playlistChoice}
  <div class="playlist-modal" role="presentation">
    <section class="playlist-dialog" aria-labelledby="playlist-title">
      <div class="playlist-titlebar">
        <h2 id="playlist-title" class="card-title grow">{playlist.title}</h2>
        <span class="muted">已选 {playlist.selected.filter(Boolean).length} / {playlist.entries.length}</span>
      </div>
      <div class="form-row">
        <input class="playlist-search grow" type="text" aria-label="搜索播放列表" placeholder="搜索标题、序号或链接" value={playlist.search} oninput={(event) => downloadStore.setPlaylistSearch(event.currentTarget.value)} />
        <button class="btn" onclick={() => downloadStore.selectPlaylistEntries(true)}>全选</button>
        <button class="btn" onclick={() => downloadStore.selectPlaylistEntries(false)}>反选</button>
      </div>
      <div class="form-row">
        <label>范围 <input type="number" min="1" max={playlist.entries.length} bind:value={rangeStart} /></label>
        <span>至</span>
        <input aria-label="范围结束序号" type="number" min="1" max={playlist.entries.length} bind:value={rangeEnd} />
        <button class="btn" onclick={() => downloadStore.selectPlaylistRange(Number(rangeStart), Number(rangeEnd))}>选择范围</button>
      </div>
      <div class="playlist-list" aria-label="播放列表条目">
        {#each downloadStore.filteredPlaylistEntries as { entry, position } (entry.index)}
          <label class="playlist-entry">
            <input type="checkbox" checked={playlist.selected[position]} onchange={() => downloadStore.togglePlaylistEntry(position)} />
            {#if entry.thumbnailUrl}<img class="playlist-thumb" src={entry.thumbnailUrl} alt="" referrerpolicy="no-referrer" />{:else}<span class="playlist-thumb playlist-thumb-empty"></span>{/if}
            <span class="playlist-index">{entry.index}</span>
            <span class="playlist-entry-copy"><span class="playlist-entry-title">{entry.title}</span><span class="playlist-entry-url">{entry.url}</span></span>
          </label>
        {/each}
      </div>
      <div class="form-row"><span class="grow"></span><button class="btn" onclick={() => (downloadStore.playlistChoice = null)}>取消</button><button class="btn-primary" disabled={!playlist.selected.some(Boolean)} onclick={() => downloadStore.confirmPlaylistSelection()}>加入并解析所选项</button></div>
    </section>
  </div>
{/if}

<dialog bind:this={importDialog} class="import-dialog" aria-labelledby="import-dialog-title" aria-describedby="import-dialog-description">
  <div class="row">
    <h2 id="import-dialog-title" class="card-title grow">批量输入链接</h2>
    <button type="button" class="btn" aria-label="关闭批量输入链接窗口" onclick={() => importDialog.close()}>关闭</button>
  </div>
  <p id="import-dialog-description" class="subtitle">每行输入一个 HTTP / HTTPS 视频链接，确认后加入下载队列。</p>
  <textarea
    bind:this={importInput}
    aria-label="批量视频链接，每行一个"
    placeholder="每行一个 HTTP / HTTPS 视频链接"
    bind:value={downloadStore.importText}
    disabled={downloadStore.isBusy}
  ></textarea>
  {#if downloadStore.importText.trim()}
    <div class="import-preview" aria-label="导入预览">
      {#each downloadStore.importPreview as entry}
        <div class="import-preview-line">
          <span class="muted">{entry.status === "valid" ? "有效" : entry.status === "duplicate" ? "重复" : "无效"}</span> · {entry.url}
        </div>
      {/each}
    </div>
  {/if}
  <div class="form-row">
    <Switch
      checked={downloadStore.keepDuplicates}
      disabled={downloadStore.isBusy}
      title="保留重复项"
      onchange={(value) => (downloadStore.keepDuplicates = value)}
    />
    <span class="muted small">保留重复项</span>
    <span class="grow"></span>
    <button type="button" class="btn" onclick={() => importDialog.close()}>取消</button>
    <button
      type="button"
      class="btn-primary"
      disabled={downloadStore.isBusy || !downloadStore.importPreview.some(entry => entry.status === "valid" || entry.status === "duplicate" && downloadStore.keepDuplicates)}
      onclick={() => { downloadStore.importLinks(); importDialog.close(); }}
    >导入队列</button>
  </div>
</dialog>

<style>
  .playlist-modal { position: fixed; inset: 0; z-index: 1000; display: grid; place-items: center; background: rgba(0, 0, 0, 0.4); }
  .playlist-dialog { width: min(760px, calc(100vw - 40px)); max-height: calc(100vh - 48px); padding: 20px; overflow: hidden; border: 1px solid var(--card-border); border-radius: 8px; background: var(--card-bg); color: var(--text-primary); box-shadow: 0 12px 48px rgba(0, 0, 0, 0.24); }
  .playlist-titlebar { display: flex; align-items: center; gap: 8px; }
  .playlist-search { min-width: 0; width: auto; }
  .playlist-list { max-height: min(48vh, 440px); overflow: auto; margin: 12px 0; border: 1px solid var(--divider); border-radius: 6px; background: var(--list-bg); }
  .playlist-entry { display: flex; align-items: center; gap: 10px; min-height: 58px; padding: 6px 10px; border-bottom: 1px solid var(--divider); cursor: pointer; }
  .playlist-entry:hover { background: var(--subtle-hover); }
  .playlist-entry input[type="checkbox"] { appearance: none; width: 16px; height: 16px; flex: 0 0 16px; margin: 0; border: 1px solid var(--control-border); border-radius: 3px; background: var(--control-bg); }
  .playlist-entry input[type="checkbox"]:checked { border-color: var(--accent); background: var(--accent); box-shadow: inset 0 0 0 3px var(--accent); }
  .playlist-index { width: 34px; color: var(--text-secondary); text-align: right; }
  .playlist-thumb { width: 72px; height: 42px; flex: 0 0 72px; object-fit: cover; border-radius: 4px; background: var(--placeholder); }
  .playlist-thumb-empty { border: 1px solid var(--control-border); }
  .playlist-entry-copy { min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .playlist-entry-title, .playlist-entry-url { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .playlist-entry-title { font-weight: 500; }
  .playlist-entry-url { color: var(--text-secondary); font-size: 12px; user-select: text; }

  .import-dialog {
    width: min(680px, calc(100vw - 40px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 20px;
    border: 1px solid var(--card-border);
    border-radius: 8px;
    background: var(--card-bg);
    color: var(--text-primary);
    box-shadow: 0 12px 48px rgba(0, 0, 0, 0.24);
  }

  .import-dialog::backdrop {
    background: rgba(0, 0, 0, 0.4);
  }

  .import-dialog textarea {
    width: 100%;
    min-height: 180px;
    margin-top: 14px;
    resize: vertical;
  }

  .import-preview {
    max-height: 180px;
    overflow: auto;
    margin-top: 12px;
    padding: 10px;
    border: 1px solid var(--divider);
    border-radius: 6px;
    background: var(--list-bg);
    user-select: text;
  }

  .import-preview-line {
    overflow-wrap: anywhere;
    line-height: 1.6;
  }
</style>
