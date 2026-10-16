// Download-page state machine. This is the TypeScript counterpart of the WPF
// `DownloadViewModel` + `QueueItemViewModel`: parsing, the in-page queue, manual
// source selection, progress/logs and the per-item lifecycle. The heavy lifting
// (arg building, JSON parsing, process handling) stays in Rust.

import * as api from "./api";
import { settingsStore } from "./store.svelte";
import {
  EXTENSION_TOKEN,
  RESOLUTION_CATALOG,
  TEMPLATE_FIELD_CATALOG,
  type AudioSourceDto,
  type CommandError,
  type CookieListingDto,
  type DownloadKind,
  type DownloadRequestDto,
  type DownloadUpdateDto,
  type MediaInfoDto,
  type PlaylistInfoDto,
  type ProxyIndicatorDto,
  type ResolutionPreference,
  type TemplateField,
  type VideoSourceDto,
} from "./types";

export type QueueItemState =
  | "Pending"
  | "Parsing"
  | "Parsed"
  | "Downloading"
  | "Completed"
  | "Failed"
  | "Cancelled";

export interface QueueItem {
  id: number;
  url: string;
  title: string;
  thumbnailUrl: string | null;
  state: QueueItemState;
  progress: number;
  outputPath: string | null;
  error: string | null;
  isLive: boolean;
  videos: VideoSourceDto[];
  audios: AudioSourceDto[];
  selectedVideo: VideoSourceDto | null;
  selectedAudio: AudioSourceDto | null;
}

const MAX_LOG_LINES = 1000;
const COMPLETION_DISPLAY_MS = 3000;

export function itemStatusText(item: QueueItem): string {
  switch (item.state) {
    case "Pending":
      return "等待解析";
    case "Parsing":
      return "正在解析…";
    case "Parsed":
      return item.isLive ? "已解析，等待下载（直播）" : "已解析，等待下载";
    case "Downloading":
      return `下载中 ${Math.round(item.progress)}%`;
    case "Completed":
      return "已完成";
    case "Failed":
      return item.error ? `失败：${item.error}` : "失败";
    case "Cancelled":
      return "已取消";
  }
}

export function itemSelectionInfo(item: QueueItem): string {
  const parts: string[] = [];

  if (item.selectedVideo) {
    const bits = [
      item.selectedVideo.resolution,
      item.selectedVideo.fpsText,
      item.selectedVideo.bitrateText,
      item.selectedVideo.codec,
    ].filter((value) => value && value.trim().length > 0);
    parts.push("视频：" + bits.join(" · "));
  }

  if (item.selectedAudio) {
    const bits = [item.selectedAudio.bitrateText, item.selectedAudio.codec].filter(
      (value) => value && value.trim().length > 0,
    );
    parts.push("音频：" + bits.join(" · "));
  }

  return parts.join("    ");
}

export function hasValidManualSelection(item: QueueItem, kind: DownloadKind): boolean {
  if (kind === "VideoOnly") return item.selectedVideo !== null;
  if (kind === "AudioOnly") return item.selectedAudio !== null;
  return (
    item.selectedVideo !== null && (item.selectedVideo.hasAudio || item.selectedAudio !== null)
  );
}

function resolutionLabel(preference: ResolutionPreference): string {
  return RESOLUTION_CATALOG.find((option) => option.value === preference)?.label ?? "最佳";
}

function samePath(a: string | null, b: string | null): boolean {
  return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

function isCancelled(error: unknown): boolean {
  return (error as CommandError | undefined)?.kind === "cancelled";
}

function errorMessage(error: unknown): string {
  const typed = error as CommandError | undefined;
  if (typed?.message) return typed.message;
  return String(error);
}

class DownloadStore {
  // --- URL / parse result -------------------------------------------------
  url = $state("");
  hasMediaInfo = $state(false);
  videoTitle = $state("");
  metaText = $state("");
  thumbnailUrl = $state<string | null>(null);
  videos = $state<VideoSourceDto[]>([]);
  audios = $state<AudioSourceDto[]>([]);
  selectedVideo = $state<VideoSourceDto | null>(null);
  selectedAudio = $state<AudioSourceDto | null>(null);
  isLive = $state(false);
  isUpcoming = $state(false);

  // --- run state ----------------------------------------------------------
  isBusy = $state(false);
  isManualMode = $state(false);
  isProgressVisible = $state(false);
  progress = $state(0);
  speedText = $state("");
  etaText = $state("");
  logs = $state<string[]>([]);
  statusText = $state("就绪：输入视频链接后点击“解析”，即可查看可用的画质与音质源。");
  ytDlpStatus = $state("");
  ytDlpReady = $state(false);

  // --- queue --------------------------------------------------------------
  queue = $state<QueueItem[]>([]);
  selectedQueueItemId = $state<number | null>(null);
  queueProgressText = $state("");

  // --- cookies ------------------------------------------------------------
  cookieListing = $state<CookieListingDto>({
    folder: "",
    exists: false,
    needsSetup: true,
    hint: "",
    files: [],
  });
  selectedCookiePath = $state<string | null>(null);

  // --- proxy --------------------------------------------------------------
  proxy = $state<ProxyIndicatorDto>({
    willApply: false,
    text: "代理：未启用",
    detail: "",
    proxyUrl: null,
  });

  // --- naming -------------------------------------------------------------
  templateFields = $state<TemplateField[]>([]);

  // --- internal -----------------------------------------------------------
  defaultDirectory = $state("");
  private nextId = 1;
  private currentItemId: number | null = null;
  private queueIndex = 0;
  private queueTotal = 0;
  private taskbarResetTimer: number | null = null;
  /** Last cookie folder the listing was built from (drives `onSettingsChanged`). */
  private lastCookieFolder = "";

  // ------------------------------------------------------------------ setup

  async initialize(): Promise<void> {
    this.defaultDirectory = await api.defaultDownloadDir();
    this.initializeTemplateFields();
    this.selectedCookiePath = settingsStore.value.CookieFile || null;
    await Promise.all([
      this.refreshCookieListing(),
      this.refreshProxyIndicator(),
      this.refreshYtDlpStatus(),
    ]);
  }

  async refreshYtDlpStatus(): Promise<void> {
    const status = await api.ytdlpStatus();
    this.ytDlpStatus = status.text;
    this.ytDlpReady = status.ready;
  }

  async refreshProxyIndicator(): Promise<void> {
    const url = this.url.trim();
    this.proxy = await api.proxyIndicator(url.length > 0 ? url : null);
  }

  async refreshCookieListing(): Promise<void> {
    const listing = await api.listCookieFiles();
    this.cookieListing = listing;
    this.lastCookieFolder = listing.folder;

    const saved = settingsStore.value.CookieFile;
    const has = (path: string | null) =>
      !!path && listing.files.some((file) => samePath(file.path, path));

    if (has(saved)) this.selectedCookiePath = saved;
    else if (has(this.selectedCookiePath)) {
      // keep the current selection
    } else {
      this.selectedCookiePath = null;
    }
  }

  /**
   * Reacts to a settings change the way `DownloadViewModel.OnSettingsChanged`
   * does: rebuild the cookie list when the folder changed, always re-check the
   * proxy indicator, and re-resolve yt-dlp.
   */
  onSettingsChanged(): void {
    const folder = settingsStore.value.CookieFolder ?? "";
    if (folder.toLowerCase() !== this.lastCookieFolder.toLowerCase()) {
      void this.refreshCookieListing();
    }
    void this.refreshProxyIndicator();
    void this.refreshYtDlpStatus();
  }

  // ------------------------------------------------------------- persisted getters

  get downloadDirectory(): string {
    return settingsStore.value.DownloadDirectory;
  }

  get downloadDirectoryInput(): string {
    const value = settingsStore.value.DownloadDirectory;
    return value.trim().length > 0 ? value : this.defaultDirectory;
  }

  get threads(): number {
    return settingsStore.value.DownloadThreads;
  }

  get downloadKind(): DownloadKind {
    return settingsStore.value.DownloadKind;
  }

  get resolution(): ResolutionPreference {
    return settingsStore.value.Resolution;
  }

  get writeThumbnail(): boolean {
    return settingsStore.value.WriteThumbnail;
  }

  get outputNameMode(): string {
    return settingsStore.value.OutputNameMode;
  }

  get fixedFileName(): string {
    return settingsStore.value.OutputFileName;
  }

  get isTemplateNameMode(): boolean {
    return settingsStore.value.OutputNameMode !== "Fixed";
  }

  get isFixedNameMode(): boolean {
    return settingsStore.value.OutputNameMode === "Fixed";
  }

  get isAutoMode(): boolean {
    return !this.isManualMode;
  }

  get selectedTokens(): string[] {
    return this.templateFields.filter((field) => field.selected).map((field) => field.token);
  }

  /** Read-only yt-dlp template preview (fields + extension). */
  get outputTemplate(): string {
    return `${this.selectedTokens.join(" ")}.${EXTENSION_TOKEN}`;
  }

  get outputNameHint(): string {
    return this.isTemplateNameMode
      ? "模板模式：点击下方字段按钮拼装文件名，再次点击可移除；扩展名（.%(ext)s）会自动添加。"
      : "文件名模式：直接输入文件名即可，无需填写扩展名，程序会自动添加。";
  }

  // ------------------------------------------------------------- derived state

  get isQueueMode(): boolean {
    return this.queue.length > 0;
  }

  get isSingleMode(): boolean {
    return !this.isQueueMode;
  }

  get showMediaCard(): boolean {
    return this.hasMediaInfo && !this.isQueueMode;
  }

  get showSourceTables(): boolean {
    return this.isManualMode && (!this.isQueueMode || this.selectedQueueItemId !== null);
  }

  get showVideoSources(): boolean {
    return this.downloadKind !== "AudioOnly";
  }

  get showAudioSources(): boolean {
    return this.downloadKind !== "VideoOnly";
  }

  get showBothSources(): boolean {
    return this.showVideoSources && this.showAudioSources;
  }

  get selectedQueueItem(): QueueItem | null {
    return this.queue.find((item) => item.id === this.selectedQueueItemId) ?? null;
  }

  get activeVideos(): VideoSourceDto[] {
    return this.isQueueMode ? (this.selectedQueueItem?.videos ?? []) : this.videos;
  }

  get activeAudios(): AudioSourceDto[] {
    return this.isQueueMode ? (this.selectedQueueItem?.audios ?? []) : this.audios;
  }

  get activeSelectedVideo(): VideoSourceDto | null {
    return this.isQueueMode ? (this.selectedQueueItem?.selectedVideo ?? null) : this.selectedVideo;
  }

  set activeSelectedVideo(value: VideoSourceDto | null) {
    if (this.isQueueMode) {
      const item = this.selectedQueueItem;
      if (item) item.selectedVideo = value;
    } else {
      this.selectedVideo = value;
    }
  }

  get activeSelectedAudio(): AudioSourceDto | null {
    return this.isQueueMode ? (this.selectedQueueItem?.selectedAudio ?? null) : this.selectedAudio;
  }

  set activeSelectedAudio(value: AudioSourceDto | null) {
    if (this.isQueueMode) {
      const item = this.selectedQueueItem;
      if (item) item.selectedAudio = value;
    } else {
      this.selectedAudio = value;
    }
  }

  get showStreamBanner(): boolean {
    return this.isLive || this.isUpcoming;
  }

  get streamBannerTitle(): string {
    return this.isLive ? "当前为直播" : "预约直播尚未开始";
  }

  get streamBannerDetail(): string {
    return this.isLive
      ? "已自动切换为直播下载模式：从头录制（--live-from-start）并使用 MPEG-TS 容器（--hls-use-mpegts），降低中断损坏风险。直播结束前请保持程序运行。"
      : "该链接为预告 / 预约直播，尚未开始，暂时无法下载。直播开始后请重新点击“解析”。";
  }

  get streamBannerKind(): "live" | "upcoming" {
    return this.isLive ? "live" : "upcoming";
  }

  get progressText(): string {
    return `${Math.round(this.progress)}%`;
  }

  get progressInfoText(): string {
    const parts: string[] = [];
    if (this.speedText) parts.push(`速度 ${this.speedText}`);
    if (this.etaText) parts.push(`剩余 ${this.etaText}`);
    return parts.join("    ");
  }

  get selectionHint(): string {
    if (this.isQueueMode) {
      const item = this.selectedQueueItem;
      return item
        ? `正在设置：${item.title}`
        : "队列模式：点击队列中的视频，即可在手动模式下为其选择视频源与音频源。";
    }

    if (this.isManualMode) {
      if (this.downloadKind === "VideoOnly") {
        return this.selectedVideo
          ? `已选视频源 ${this.selectedVideo.formatId}（仅视频，无声音）`
          : "仅视频：请从上方列表选择一个视频源（仅下载画面，不含声音）。";
      }
      if (this.downloadKind === "AudioOnly") {
        return this.selectedAudio
          ? `已选音频源 ${this.selectedAudio.formatId}（仅音频）`
          : "仅音频：请从上方列表选择一个音频源。";
      }
      if (this.manualSelectionValid) {
        return this.selectedVideo!.hasAudio
          ? `已选视频源 ${this.selectedVideo!.formatId}（音视频合一，单文件下载）`
          : `已选：${this.selectedVideo!.formatId} + ${this.selectedAudio!.formatId}`;
      }
      return "手动模式：请从上方列表选择一个视频源；纯视频源需再选择一个音频源用于合成。";
    }

    const suffix =
      this.resolution === "Best" ? "" : `（分辨率限制 ${resolutionLabel(this.resolution)}）`;
    if (this.downloadKind === "VideoOnly") {
      return `仅视频：自动挑选最高画质视频源${suffix}，不下载声音。`;
    }
    if (this.downloadKind === "AudioOnly") {
      return "仅音频：自动挑选最高音质的音频源。";
    }
    return `自动模式：自动挑选最佳视频与音频并合成${suffix}。`;
  }

  get manualSelectionValid(): boolean {
    if (this.downloadKind === "VideoOnly") return this.selectedVideo !== null;
    if (this.downloadKind === "AudioOnly") return this.selectedAudio !== null;
    return (
      this.selectedVideo !== null && (this.selectedVideo.hasAudio || this.selectedAudio !== null)
    );
  }

  get canParse(): boolean {
    if (this.isBusy || !this.ytDlpReady) return false;
    if (this.isQueueMode) {
      return this.queue.some((item) => item.state === "Pending" || item.state === "Failed");
    }
    return this.url.trim().length > 0;
  }

  get canDownload(): boolean {
    if (this.isBusy || !this.ytDlpReady) return false;
    if (this.isQueueMode) {
      return this.queue.some((item) =>
        ["Pending", "Parsed", "Failed", "Cancelled"].includes(item.state),
      );
    }
    return (
      !this.isUpcoming &&
      this.url.trim().length > 0 &&
      (this.isAutoMode || this.manualSelectionValid)
    );
  }

  get canCancel(): boolean {
    return this.isBusy;
  }

  get canAddToQueue(): boolean {
    return !this.isBusy && this.url.trim().length > 0 && this.ytDlpReady;
  }

  get canClearQueue(): boolean {
    return !this.isBusy && this.queue.length > 0;
  }

  // ------------------------------------------------------------- field setters

  setUrl(value: string): void {
    if (this.url === value) return;
    this.url = value;
    this.clearParseResult();
    void this.refreshProxyIndicator();
  }

  setDownloadDirectory(value: string): void {
    settingsStore.setLocal("DownloadDirectory", value);
    settingsStore.persist({ DownloadDirectory: value.trim() });
  }

  async browseFolder(): Promise<void> {
    const picked = await api.pickFolder("选择下载保存目录");
    if (picked) this.setDownloadDirectory(picked);
  }

  setThreads(value: number): void {
    const clamped = Math.min(64, Math.max(1, Math.round(value) || 1));
    settingsStore.set("DownloadThreads", clamped);
  }

  setManualMode(manual: boolean): void {
    this.isManualMode = manual;
    if (manual) this.ensureActiveSelection();
  }

  ensureActiveSelection(): void {
    if (!this.isQueueMode) {
      if (!this.selectedVideo && this.videos.length > 0) this.selectedVideo = this.videos[0];
      if (this.downloadKind === "Default" && !this.selectedAudio && this.audios.length > 0) {
        this.selectedAudio = this.audios[0];
      }
    }
  }

  setDownloadKind(kind: DownloadKind): void {
    if (this.downloadKind === kind) return;
    settingsStore.set("DownloadKind", kind);
  }

  setResolution(preference: ResolutionPreference): void {
    settingsStore.set("Resolution", preference);
  }

  setWriteThumbnail(value: boolean): void {
    settingsStore.set("WriteThumbnail", value);
  }

  setNameMode(template: boolean): void {
    settingsStore.set("OutputNameMode", template ? "Template" : "Fixed");
  }

  setFixedFileName(value: string): void {
    settingsStore.setLocal("OutputFileName", value);
    settingsStore.persist({ OutputFileName: value.trim() });
  }

  setCookie(path: string | null): void {
    this.selectedCookiePath = path;
    settingsStore.set("CookieFile", path ?? "");
  }

  initializeTemplateFields(): void {
    const saved = settingsStore.value.OutputTemplate ?? "";
    this.templateFields = TEMPLATE_FIELD_CATALOG.map((field) => ({
      ...field,
      selected: saved.length > 0 && saved.toLowerCase().includes(field.token.toLowerCase()),
    }));
    if (!this.templateFields.some((field) => field.selected)) {
      const title = this.templateFields.find((field) => field.token === "%(title)s");
      if (title) title.selected = true;
    }
    this.persistTemplate();
  }

  toggleTemplateField(token: string): void {
    const field = this.templateFields.find((entry) => entry.token === token);
    if (!field) return;

    field.selected = !field.selected;
    if (!this.templateFields.some((entry) => entry.selected)) {
      const title = this.templateFields.find((entry) => entry.token === "%(title)s");
      if (title) title.selected = true;
    }
    this.persistTemplate();
  }

  private persistTemplate(): void {
    settingsStore.set(
      "OutputTemplate",
      this.templateFields
        .filter((field) => field.selected)
        .map((field) => field.token)
        .join(" "),
    );
  }

  private buildOutputTemplate(): string | null {
    if (this.isFixedNameMode) {
      const name = this.fixedFileName.trim();
      if (name.length === 0) return null;
      return name.toLowerCase().includes(EXTENSION_TOKEN.toLowerCase())
        ? name
        : `${name}.${EXTENSION_TOKEN}`;
    }
    const template = this.outputTemplate.trim();
    return template.length === 0 ? null : template;
  }

  // ------------------------------------------------------------- parse

  clearParseResult(): void {
    this.videos = [];
    this.audios = [];
    this.videoTitle = "";
    this.metaText = "";
    this.thumbnailUrl = null;
    this.hasMediaInfo = false;
    this.isLive = false;
    this.isUpcoming = false;
    this.selectedVideo = null;
    this.selectedAudio = null;
  }

  async parse(): Promise<void> {
    if (this.isQueueMode) {
      await this.parseQueue();
      return;
    }

    this.isBusy = true;
    this.statusText = "正在解析视频信息…";

    let playlist: PlaylistInfoDto | null = null;
    try {
      const result = await api.parseMedia(this.url.trim());
      if (result.playlist) {
        playlist = result.playlist;
      } else if (result.media) {
        this.applyMediaInfo(result.media);
      } else {
        this.statusText = "解析失败：未获得视频信息。";
      }
    } catch (error) {
      this.statusText = isCancelled(error) ? "解析已取消" : parseErrorMessage(error);
    } finally {
      this.isBusy = false;
    }

    if (!playlist) return;

    if (playlist.entries.length === 0) {
      this.statusText = `解析失败：播放列表“${playlist.title}”中没有可下载的视频。`;
      return;
    }

    this.expandPlaylistToQueue(playlist);
    await this.parseQueue();
  }

  private applyMediaInfo(info: MediaInfoDto): void {
    this.clearParseResult();
    this.videoTitle = info.title;
    this.metaText =
      `ID: ${info.id}    时长: ${info.durationText}` +
      (info.uploader ? `    上传者: ${info.uploader}` : "");
    this.hasMediaInfo = true;
    this.videos = info.videos;
    this.audios = info.audios;
    this.thumbnailUrl = info.thumbnailUrl;
    this.isLive = info.isLive;
    this.isUpcoming = info.isUpcoming;
    this.statusText = info.isUpcoming
      ? `解析完成：${info.title}（预约 / 预告直播，尚未开始）`
      : info.isLive
        ? `解析完成：${info.title}（当前为直播，已切换为直播下载模式）`
        : `解析完成：${info.title}（${info.videos.length} 个视频源，${info.audios.length} 个音频源）`;
  }

  private expandPlaylistToQueue(playlist: PlaylistInfoDto): void {
    for (const entry of playlist.entries) {
      if (!entry.url.trim()) continue;
      this.queue.push(this.makeItem(entry.url, entry.title));
    }

    this.url = "";
    this.statusText = `检测到播放列表“${playlist.title}”：已加入 ${playlist.entries.length} 个视频，正在逐个解析…`;
  }

  private makeItem(url: string, title?: string): QueueItem {
    return {
      id: this.nextId++,
      url,
      title: title && title.trim() ? title : url,
      thumbnailUrl: null,
      state: "Pending",
      progress: 0,
      outputPath: null,
      error: null,
      isLive: false,
      videos: [],
      audios: [],
      selectedVideo: null,
      selectedAudio: null,
    };
  }

  private applyInfoToItem(item: QueueItem, info: MediaInfoDto): void {
    item.isLive = info.isLive;
    if (info.title.trim()) item.title = info.title;
    item.thumbnailUrl = info.thumbnailUrl;
    item.error = null;
    item.videos = info.videos;
    item.audios = info.audios;
    item.state = "Parsed";
  }

  private async parseQueue(): Promise<void> {
    const targets = this.queue.filter(
      (item) => item.state === "Pending" || item.state === "Failed",
    );
    if (targets.length === 0) {
      this.statusText = "队列中的视频都已解析。";
      return;
    }

    this.isBusy = true;
    this.isProgressVisible = true;
    this.progress = 0;
    this.speedText = "";
    this.etaText = "";

    try {
      for (let index = 0; index < targets.length; index += 1) {
        const item = targets[index];
        this.queueProgressText = `解析 ${index + 1} / ${targets.length}`;
        this.statusText = `正在解析第 ${index + 1} / ${targets.length} 个：${item.url}`;
        item.state = "Parsing";

        try {
          const info = await api.getMediaInfo(item.url);
          if (info.isUpcoming) {
            item.error = "预约直播尚未开始";
            item.state = "Failed";
          } else {
            this.applyInfoToItem(item, info);
          }
        } catch (error) {
          if (isCancelled(error)) {
            item.state = "Cancelled";
            throw error;
          }
          item.error = errorMessage(error);
          item.state = "Failed";
        }

        this.progress = ((index + 1) * 100) / targets.length;
      }

      this.statusText = `解析完成：已解析 ${targets.length} 个视频，可点击“开始下载”。`;
    } catch (error) {
      this.statusText = isCancelled(error) ? "解析已取消" : `解析失败：${errorMessage(error)}`;
    } finally {
      this.isProgressVisible = false;
      this.queueProgressText = "";
      this.isBusy = false;
    }
  }

  // ------------------------------------------------------------- download

  async download(): Promise<void> {
    if (this.isQueueMode) {
      await this.downloadQueue();
      return;
    }

    this.isBusy = true;
    this.progress = 0;
    this.isProgressVisible = true;
    this.speedText = "";
    this.etaText = "";
    this.logs = [];
    this.statusText = this.isLive
      ? "开始直播下载（从头录制，使用 MPEG-TS 容器；直播结束前请保持程序运行）…"
      : this.autoStatusText();
    this.startTaskbar();

    let success = false;
    let cancelled = false;
    let error: string | null = null;
    let outputPath: string | null = null;

    this.currentItemId = null;

    try {
      const outcome = await api.startDownload(
        this.buildRequest(
          this.url.trim(),
          this.selectedVideo,
          this.selectedAudio,
          this.isManualMode && this.manualSelectionValid,
          this.buildOutputTemplate(),
        ),
      );

      if (outcome.cancelled) {
        cancelled = true;
        this.statusText = "下载已取消";
      } else if (outcome.success) {
        success = true;
        outputPath = outcome.outputPath;
        this.statusText = outputPath ? `下载完成：${outputPath}` : "下载完成";
      } else {
        error = outcome.error ?? "未知错误";
        this.statusText = `下载失败：${error}`;
        this.appendLog(error);
      }
    } catch (caught) {
      if (isCancelled(caught)) {
        cancelled = true;
        this.statusText = "下载已取消";
      } else {
        error = errorMessage(caught);
        this.statusText = `下载失败：${error}`;
      }
    } finally {
      this.isProgressVisible = false;
      this.isBusy = false;
    }

    this.finishTaskbar(success, cancelled);
    this.notifyFinished(success, cancelled, error, outputPath, this.videoTitle, this.url.trim());
  }

  private autoStatusText(): string {
    if (!this.isAutoMode) return `开始下载（格式 ${this.manualExpression()}）…`;
    switch (this.downloadKind) {
      case "VideoOnly":
        return "开始下载（仅视频，自动选择最高画质）…";
      case "AudioOnly":
        return "开始下载（仅音频，自动选择最高音质）…";
      default:
        return "开始下载（自动选择最佳质量并合成）…";
    }
  }

  private manualExpression(): string {
    if (this.downloadKind === "VideoOnly") return this.selectedVideo?.formatId ?? "";
    if (this.downloadKind === "AudioOnly") return this.selectedAudio?.formatId ?? "";

    const video = this.selectedVideo;
    if (!video) return "";
    return video.hasAudio
      ? video.formatId
      : `${video.formatId}+${this.selectedAudio?.formatId ?? ""}`;
  }

  private async downloadQueue(): Promise<void> {
    const targets = this.queue.filter((item) =>
      ["Pending", "Parsed", "Failed", "Cancelled"].includes(item.state),
    );
    if (targets.length === 0) {
      this.statusText = "队列中没有可下载的任务，请先加入队列并解析。";
      return;
    }

    const template =
      this.isFixedNameMode && targets.length > 1 ? null : this.buildOutputTemplate();

    this.isBusy = true;
    this.progress = 0;
    this.isProgressVisible = true;
    this.speedText = "";
    this.etaText = "";
    this.logs = [];
    this.startTaskbar();

    this.queueTotal = targets.length;
    let completed = 0;
    let failed = 0;
    let cancelled = false;

    try {
      for (let index = 0; index < targets.length; index += 1) {
        const item = targets[index];
        this.queueProgressText = `下载 ${index + 1} / ${targets.length}`;
        this.statusText = `正在下载第 ${index + 1} / ${targets.length} 个：${item.title}`;

        item.state = "Downloading";
        item.progress = 0;
        item.error = null;
        this.progress = 0;
        this.queueIndex = index;
        this.currentItemId = item.id;

        const useManual = this.isManualMode && hasValidManualSelection(item, this.downloadKind);
        const request = this.buildRequest(
          item.url,
          item.selectedVideo,
          item.selectedAudio,
          useManual,
          template,
        );
        request.live = item.isLive;

        try {
          const outcome = await api.startDownload(request);
          if (outcome.cancelled) {
            cancelled = true;
            item.state = "Cancelled";
            this.notifyFinished(false, true, null, null, item.title, item.url);
          } else if (outcome.success) {
            completed += 1;
            item.outputPath = outcome.outputPath;
            item.progress = 100;
            item.state = "Completed";
            this.notifyFinished(true, false, null, outcome.outputPath, item.title, item.url);
          } else {
            failed += 1;
            item.error = outcome.error ?? null;
            item.state = "Failed";
            this.notifyFinished(false, false, item.error, null, item.title, item.url);
          }
        } catch (caught) {
          if (isCancelled(caught)) {
            item.state = "Cancelled";
            throw caught;
          }
          failed += 1;
          item.error = errorMessage(caught);
          item.state = "Failed";
          this.notifyFinished(false, false, item.error, null, item.title, item.url);
        }
      }

      this.statusText = `队列下载完成：成功 ${completed} 个，失败 ${failed} 个。`;
    } catch (caught) {
      cancelled = isCancelled(caught);
      this.statusText = cancelled ? "队列下载已取消" : `下载失败：${errorMessage(caught)}`;
    } finally {
      this.isProgressVisible = false;
      this.queueProgressText = "";
      this.queueTotal = 0;
      this.currentItemId = null;
      this.isBusy = false;
    }

    // Taskbar: show the aggregate result once the queue finished.
    this.finishTaskbar(completed > 0, cancelled);
    if (!cancelled && completed === 0 && failed > 0) {
      void api.setTaskbarProgress("error", 100);
    }
  }

  private buildRequest(
    url: string,
    video: VideoSourceDto | null,
    audio: AudioSourceDto | null,
    manual: boolean,
    outputTemplate: string | null,
  ): DownloadRequestDto {
    const manualPayload =
      manual && (video !== null || audio !== null)
        ? {
            videoFormatId: video?.formatId ?? null,
            videoHasAudio: video?.hasAudio ?? false,
            audioFormatId: audio?.formatId ?? null,
          }
        : null;

    return {
      url,
      auto: !manual,
      kind: this.downloadKind,
      resolution: this.resolution,
      manual: manualPayload,
      outputDirectory: this.downloadDirectory.trim(),
      outputTemplate,
      writeThumbnail: this.writeThumbnail,
      live: this.isLive,
    };
  }

  // ------------------------------------------------------------- queue actions

  addToQueue(): void {
    const url = this.url.trim();
    if (!url) return;

    this.queue.push(this.makeItem(url));
    this.url = "";
    this.statusText =
      this.queue.length === 1
        ? "已加入队列。继续粘贴链接可添加更多，点击“解析”获取每个视频的封面与标题。"
        : `已加入队列（共 ${this.queue.length} 个），点击“开始下载”可批量下载。`;
    void this.refreshProxyIndicator();
  }

  clearQueue(): void {
    if (this.isBusy) return;
    this.queue = [];
    this.selectedQueueItemId = null;
    this.queueProgressText = "";
    this.statusText = "已清空下载队列。";
  }

  removeQueueItem(id: number): void {
    if (this.isBusy) return;
    this.queue = this.queue.filter((item) => item.id !== id);
    if (this.selectedQueueItemId === id) this.selectedQueueItemId = null;
    if (this.queue.length === 0) this.statusText = "队列已空。";
  }

  // ------------------------------------------------------------- progress

  /** Handles one `download:update` event from the backend. */
  handleUpdate(update: DownloadUpdateDto): void {
    if (update.percent !== null && update.percent !== undefined) this.progress = update.percent;
    if (update.speed) this.speedText = update.speed;
    if (update.eta) this.etaText = update.eta;
    if (update.line) this.appendLog(update.line);

    if (this.currentItemId !== null) {
      const item = this.queue.find((entry) => entry.id === this.currentItemId);
      if (item && update.percent !== null && update.percent !== undefined) {
        item.progress = update.percent;
      }
      if (this.queueTotal > 0) {
        const itemPercent = update.percent ?? item?.progress ?? 0;
        this.progress = ((this.queueIndex + itemPercent / 100) / this.queueTotal) * 100;
      }
    }

    this.updateTaskbar();
  }

  cancel(): void {
    this.statusText = "正在取消…";
    void api.cancelJob();
  }

  private appendLog(line: string): void {
    this.logs.push(line);
    if (this.logs.length > MAX_LOG_LINES) this.logs.splice(0, this.logs.length - MAX_LOG_LINES);
  }

  // ------------------------------------------------------------- taskbar / notify

  private startTaskbar(): void {
    this.cancelTaskbarReset();
    void api.setTaskbarProgress("normal", Math.round(this.progress));
  }

  private updateTaskbar(): void {
    if (this.isProgressVisible) {
      void api.setTaskbarProgress("normal", Math.round(this.progress));
    }
  }

  private finishTaskbar(success: boolean, cancelled: boolean): void {
    if (cancelled) {
      void api.setTaskbarProgress("none", null);
      return;
    }
    if (success) {
      void api.setTaskbarProgress("normal", 100);
      this.taskbarResetTimer = window.setTimeout(() => {
        void api.setTaskbarProgress("none", null);
        this.taskbarResetTimer = null;
      }, COMPLETION_DISPLAY_MS);
    } else {
      void api.setTaskbarProgress("error", 100);
    }
  }

  private cancelTaskbarReset(): void {
    if (this.taskbarResetTimer !== null) {
      window.clearTimeout(this.taskbarResetTimer);
      this.taskbarResetTimer = null;
    }
  }

  private notifyFinished(
    success: boolean,
    cancelled: boolean,
    error: string | null,
    outputPath: string | null,
    title: string,
    _url: string,
  ): void {
    if (cancelled) return;

    if (success) {
      const body = title.trim() ? `《${title}》已保存` : savedPathText(outputPath);
      void api.notify("下载完成", body);
    } else {
      void api.notify("下载失败", error ?? "未知错误");
    }
  }
}

function savedPathText(path: string | null): string {
  if (!path || !path.trim()) return "文件已保存";
  const parts = path.split(/[\\/]/);
  const name = parts[parts.length - 1];
  return name ? `已保存到 ${name}` : `已保存到 ${path}`;
}

function parseErrorMessage(error: unknown): string {
  const typed = error as CommandError | undefined;
  if (typed?.kind === "message") return typed.message;
  if (typed?.kind === "io") return `解析失败：${typed.message}`;
  return `解析失败：${errorMessage(error)}`;
}

export const downloadStore = new DownloadStore();