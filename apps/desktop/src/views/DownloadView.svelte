<script lang="ts">
  import Chip from "../lib/components/Chip.svelte";
  import ProgressBar from "../lib/components/ProgressBar.svelte";
  import Radio from "../lib/components/Radio.svelte";
  import Segmented from "../lib/components/Segmented.svelte";
  import Select from "../lib/components/Select.svelte";
  import Switch from "../lib/components/Switch.svelte";
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
</script>

<div class="page">
  <!-- 下载设置 -->
  <div class="card">
    <!-- 视频链接 -->
    <div class="form-row" style="margin-top:0">
      <span class="field-label" style="width:96px">视频链接</span>
      <input
        type="text"
        class="grow"
        style="min-width:200px"
        value={downloadStore.url}
        oninput={(event) => downloadStore.setUrl(event.currentTarget.value)}
      />
      <button
        type="button"
        class="btn-primary"
        disabled={!downloadStore.canParse}
        title="解析当前链接；若为播放列表（视频队列），会自动把列表中的全部视频加入下载队列并逐个解析；队列模式下按顺序解析队列中的所有视频"
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
        取消
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
          <img class="cover cover-160" src={downloadStore.thumbnailUrl} alt="" />
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
        <div class="card" style="flex:1; padding:12px; min-width:0">
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
        <div class="card" style="flex:1; padding:12px; min-width:0">
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
                <img class="cover cover-112" src={item.thumbnailUrl} alt="" />
              {/if}
              <div class="queue-body">
                <div class="queue-title" title={item.url}>{item.title}</div>
                <div class="muted ellipsis" style="font-size:12px; margin-top:3px">
                  {itemStatusText(item)}
                </div>
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
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <details class="logs">
      <summary>日志</summary>
      <div class="log" style="margin-top:8px">
        {#each downloadStore.logs as line, index (index)}
          <div class="log-line">{line}</div>
        {/each}
      </div>
    </details>
  </div>
</div>