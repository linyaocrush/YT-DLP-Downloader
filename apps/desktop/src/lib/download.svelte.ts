// Download-page state machine. This is the TypeScript counterpart of the WPF
// `DownloadViewModel` + `QueueItemViewModel`: parsing, the in-page queue, manual
// source selection, progress/logs and the per-item lifecycle. The heavy lifting
// (arg building, JSON parsing, process handling) stays in Rust.

import * as api from "./api";
import { previewLinks, linkKey } from "./importLinks";
import { settingsStore } from "./store.svelte";
import {
  EXTENSION_TOKEN,
  RESOLUTION_CATALOG,
  TEMPLATE_FIELD_CATALOG,
  type AudioSourceDto,
  type AppSettings,
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
  playlistIndex?: number;
  savedRequest?: DownloadRequestDto;
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

export interface PlaylistChoice extends PlaylistInfoDto {
  selected: boolean[];
  search: string;
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
  const videoValid = item.selectedVideo !== null && item.videos.some(v => v.formatId === item.selectedVideo?.formatId);
  const audioValid = item.selectedAudio !== null && item.audios.some(a => a.formatId === item.selectedAudio?.formatId);
  if (kind === "VideoOnly") return videoValid;
  if (kind === "AudioOnly") return audioValid;
  return (
    videoValid && (item.selectedVideo!.hasAudio || audioValid)
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

export class DownloadStore {
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
  liveStatus = $state<string | null>(null);

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
  private stopRequested = false;
  private parseRevision = 0;
  private activeJobId: number | null = null;
  private lastJobId = 0;
  private runTargetIds: number[] = [];
  private completedTargetIds = new Set<number>();
  private queueSaveChain: Promise<void> = Promise.resolve();
  private queueLoaded = false;
  lastOutputPath = $state<string | null>(null);
  importText = $state("");
  playlistChoice = $state<PlaylistChoice | null>(null);
  keepDuplicates = $state(false);
  saveLog = $state(false);
  get importPreview() { return previewLinks(this.importText, this.queue.map(item => item.url)); }

  get filteredPlaylistEntries() {
    const choice = this.playlistChoice;
    if (!choice) return [];
    const query = choice.search.trim().toLocaleLowerCase();
    return choice.entries.map((entry, position) => ({ entry, position }))
      .filter(({ entry }) => !query || `${entry.index} ${entry.title} ${entry.url}`.toLocaleLowerCase().includes(query));
  }

  setPlaylistSearch(value: string): void {
    if (this.playlistChoice) this.playlistChoice.search = value;
  }

  togglePlaylistEntry(position: number): void {
    const choice = this.playlistChoice;
    if (choice && position >= 0 && position < choice.selected.length) choice.selected[position] = !choice.selected[position];
  }

  selectPlaylistEntries(selected: boolean): void {
    const choice = this.playlistChoice;
    if (!choice) return;
    for (const { position } of this.filteredPlaylistEntries) choice.selected[position] = selected;
  }

  selectPlaylistRange(from: number, to: number): void {
    const choice = this.playlistChoice;
    if (!choice) return;
    const start = Math.max(1, Math.min(choice.entries.length, Math.min(from, to)));
    const end = Math.max(1, Math.min(choice.entries.length, Math.max(from, to)));
    choice.selected = choice.entries.map((entry, i) => entry.index >= start && entry.index <= end);
  }

  confirmPlaylistSelection(): void {
    const choice = this.playlistChoice;
    if (!choice) return;
    const selected = choice.entries.filter((_, position) => choice.selected[position]);
    if (!selected.length) { this.statusText = "请至少选择一个播放列表条目。"; return; }
    for (const entry of selected) {
      if (!entry.url.trim() || this.queue.some(item => linkKey(item.url) === linkKey(entry.url))) continue;
      const item = this.makeItem(entry.url, entry.title, entry.thumbnailUrl);
      item.playlistIndex = entry.index;
      this.queue.push(item);
    }
    this.playlistChoice = null;
    this.url = "";
    this.statusText = `已加入 ${selected.length} 个选中视频，正在逐个解析…`;
    void this.persistQueue();
    void this.parseQueue();
  }

  confirmExternalPlaylistSelection(entries: PlaylistInfoDto["entries"]): void {
    if (!this.playlistChoice) return;
    const selected = new Set(entries.map(entry => entry.index));
    for (const entry of this.playlistChoice.entries) {
      if (!selected.has(entry.index) || !entry.url.trim() || this.queue.some(item => linkKey(item.url) === linkKey(entry.url))) continue;
      const item = this.makeItem(entry.url, entry.title, entry.thumbnailUrl);
      item.playlistIndex = entry.index;
      this.queue.push(item);
    }
    const count = selected.size;
    this.playlistChoice = null;
    this.url = "";
    this.statusText = `已加入 ${count} 个选中视频，正在逐个解析…`;
    void this.persistQueue();
    void this.parseQueue();
  }

  importLinks(): void {
    const entries = this.importPreview.filter(entry => entry.status === "valid" || (entry.status === "duplicate" && this.keepDuplicates));
    for (const entry of entries) this.queue.push(this.makeItem(entry.url));
    this.statusText = `已导入 ${entries.length} 个链接`;
    this.importText = "";
    void this.persistQueue();
  }
  private currentItemId: number | null = null;
  private queueIndex = 0;
  private queueTotal = 0;
  private taskbarResetTimer: number | null = null;
  /** Last cookie folder the listing was built from (drives `onSettingsChanged`). */
  private lastCookieFolder = "";

  // ------------------------------------------------------------------ setup

  async initialize(): Promise<void> {
    try {
      this.queue = await api.loadQueue() as QueueItem[];
      this.queueLoaded = true;
      this.nextId = Math.max(0, ...this.queue.map(item => item.id)) + 1;
      if (this.queue.length) this.statusText = `已恢复 ${this.queue.length} 个任务（包含下载历史），手动格式需重新解析选择。`;
    } catch (error) { this.statusText = errorMessage(error); }
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
      if (saved) await settingsStore.set("CookieFile", "");
    }
  }

  /**
   * Reacts to a settings change the way `DownloadViewModel.OnSettingsChanged`
   * does: rebuild the cookie list when the folder changed, always re-check the
   * proxy indicator, and re-resolve yt-dlp.
   */
  onSettingsChanged(keys: (keyof AppSettings)[] = []): void {
    const folder = settingsStore.value.CookieFolder ?? "";
    if (folder.toLowerCase() !== this.lastCookieFolder.toLowerCase()) {
      void this.refreshCookieListing();
    }
    if (keys.some(key => key.startsWith("Proxy"))) void this.refreshProxyIndicator();
    if (keys.includes("YtDlpPath")) void this.refreshYtDlpStatus();
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

  get isPostLive(): boolean {
    return this.liveStatus?.toLowerCase() === "post_live";
  }

  get hasPlayableFormats(): boolean {
    return this.videos.length > 0 || this.audios.length > 0;
  }

  get showStreamBanner(): boolean {
    return this.isLive || this.isUpcoming || this.isPostLive;
  }

  get streamBannerTitle(): string {
    if (this.isLive) return "当前为直播";
    if (this.isPostLive) return "直播已结束";
    return "预约直播尚未开始";
  }

  get streamBannerDetail(): string {
    if (this.isLive) {
      return "已自动切换为直播下载模式：从头录制（--live-from-start）并使用 MPEG-TS 容器（--hls-use-mpegts），降低中断损坏风险。直播结束前请保持程序运行。";
    }
    if (this.isPostLive) {
      return this.hasPlayableFormats
        ? "YouTube 已生成回放，可以开始下载。若下载失败，请稍后重新解析。"
        : "直播刚刚结束，YouTube 仍在生成回放，暂时拿不到可下载格式。通常需要几分钟到数小时，请稍后重新点击“解析”。";
    }
    return "该链接为预告 / 预约直播，尚未开始，暂时无法下载。直播开始后请重新点击“解析”。";
  }

  get streamBannerKind(): "live" | "upcoming" | "post_live" {
    if (this.isLive) return "live";
    if (this.isPostLive) return "post_live";
    return "upcoming";
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
      (!this.hasMediaInfo || this.hasPlayableFormats) &&
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
    this.parseRevision += 1;
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
    for (const item of this.queue) {
      if (item.state !== "Completed" && item.savedRequest) {
        item.savedRequest.auto = !manual;
        item.savedRequest.manual = null;
      }
    }
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
    this.liveStatus = null;
    this.selectedVideo = null;
    this.selectedAudio = null;
  }

  async parse(): Promise<void> {
    if (this.isBusy) return;
    this.activeJobId = null;
    await settingsStore.flush();
    if (settingsStore.error) { this.statusText = settingsStore.error; return; }
    this.stopRequested = false;
    if (this.isQueueMode) {
      await this.parseQueue();
      return;
    }

    const revision = ++this.parseRevision;
    const url = this.url.trim();
    this.isBusy = true;
    this.statusText = "正在解析视频信息…";

    let playlist: PlaylistInfoDto | null = null;
    try {
      const result = await api.parseMedia(url, this.selectedCookiePath);
      if (revision !== this.parseRevision || this.stopRequested) return;
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
      await this.persistQueue();
    }

    if (!playlist || this.stopRequested) return;

    if (playlist.entries.length === 0) {
      this.statusText = `解析失败：播放列表“${playlist.title}”中没有可下载的视频。`;
      return;
    }

    this.playlistChoice = { ...playlist, selected: playlist.entries.map(() => false), search: "" };
    this.statusText = `播放列表“${playlist.title}”共 ${playlist.entries.length} 项，请选择要下载的内容。`;
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
    this.liveStatus = info.liveStatus;
    const postLive = info.liveStatus?.toLowerCase() === "post_live";
    const hasFormats = info.videos.length > 0 || info.audios.length > 0;
    this.statusText = info.isUpcoming
      ? `解析完成：${info.title}（预约 / 预告直播，尚未开始）`
      : info.isLive
        ? `解析完成：${info.title}（当前为直播，已切换为直播下载模式）`
        : postLive
          ? hasFormats
            ? `解析完成：${info.title}（直播已结束，回放格式已可用）`
            : `解析完成：${info.title}（直播刚结束，YouTube 正在生成回放，暂时无法下载；请稍后重新解析）`
          : hasFormats
            ? `解析完成：${info.title}（${info.videos.length} 个视频源，${info.audios.length} 个音频源）`
            : `解析完成：${info.title}（未找到可下载格式，请稍后重新解析）`;
  }

  private makeItem(url: string, title?: string, thumbnailUrl?: string | null): QueueItem {
    return {
      id: this.nextId++,
      url,
      title: title && title.trim() ? title : url,
      thumbnailUrl: thumbnailUrl ?? null,
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
    item.selectedVideo = info.videos.find(v => v.formatId === item.selectedVideo?.formatId) ?? null;
    item.selectedAudio = info.audios.find(a => a.formatId === item.selectedAudio?.formatId) ?? null;

    const postLive = info.liveStatus?.toLowerCase() === "post_live";
    if (postLive && info.videos.length === 0 && info.audios.length === 0) {
      item.error = "直播刚结束，YouTube 正在生成回放，暂时无法下载；请稍后重新解析。";
      item.state = "Failed";
      return;
    }

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
        if (this.stopRequested) throw { kind: "cancelled" };
        const item = targets[index];
        this.queueProgressText = `解析 ${index + 1} / ${targets.length}`;
        this.statusText = `正在解析第 ${index + 1} / ${targets.length} 个：${item.url}`;
        item.state = "Parsing";

        try {
          const info = await api.getMediaInfo(item.url, this.selectedCookiePath);
          if (this.stopRequested) throw { kind: "cancelled" };
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
      await this.persistQueue();
    }
  }

  // ------------------------------------------------------------- download

  /// Re-checks a live URL right before downloading: a stream that was live
  /// when parsed may have ended since. Returns false when there is nothing
  /// downloadable yet (post-live replay still being generated).
  private async refreshLiveState(url: string, item?: QueueItem): Promise<boolean> {
    try {
      const info = await api.getMediaInfo(url, this.selectedCookiePath);
      const postLive = info.liveStatus?.toLowerCase() === "post_live";
      const hasFormats = info.videos.length > 0 || info.audios.length > 0;

      if (item) {
        item.isLive = info.isLive;
        item.videos = info.videos;
        item.audios = info.audios;
      } else {
        this.isLive = info.isLive;
        this.isUpcoming = info.isUpcoming;
        this.liveStatus = info.liveStatus;
        this.videos = info.videos;
        this.audios = info.audios;
        if (
          this.selectedVideo &&
          !this.videos.some((video) => video.formatId === this.selectedVideo!.formatId)
        ) {
          this.selectedVideo = null;
        }
        if (
          this.selectedAudio &&
          !this.audios.some((audio) => audio.formatId === this.selectedAudio!.formatId)
        ) {
          this.selectedAudio = null;
        }
      }

      if (postLive && !hasFormats) {
        const message =
          "直播刚结束，YouTube 正在生成回放，暂时无法下载；请稍后重新解析。";
        if (item) {
          item.error = message;
          item.state = "Failed";
        } else {
          this.statusText = message;
        }
        return false;
      }
    } catch (error) {
      if (isCancelled(error) || this.stopRequested) throw { kind: "cancelled" };
      // Keep the previous state; the actual download will surface the error.
    }
    return true;
  }

  async download(): Promise<void> {
    if (this.isBusy) return;
    this.activeJobId = null;
    await settingsStore.flush();
    if (settingsStore.error) { this.statusText = settingsStore.error; return; }
    this.stopRequested = false;
    if (this.isQueueMode) {
      await this.downloadQueue();
      return;
    }

    // Single downloads use the same durable scheduler and history as batch tasks.
    const item = this.makeItem(this.url.trim(), this.videoTitle);
    item.videos = this.videos;
    item.audios = this.audios;
    item.selectedVideo = this.selectedVideo;
    item.selectedAudio = this.selectedAudio;
    item.isLive = this.isLive;
    this.queue.push(item);
    await this.downloadQueue([item.id]);
  }

  private async downloadQueue(ids?: number[]): Promise<void> {
    this.activeJobId = null;
    const targets = this.queue.filter((item) => (!ids || ids.includes(item.id)) &&
      ["Pending", "Parsed", "Failed", "Cancelled"].includes(item.state),
    );
    if (targets.length === 0) {
      this.statusText = "队列中没有可下载的任务，请先加入队列并解析。";
      return;
    }

    if (this.isManualMode || targets.some(item => item.savedRequest?.auto === false)) {
      const invalid = targets.filter(item => (item.savedRequest ? !item.savedRequest.auto : this.isManualMode) && !hasValidManualSelection(item, item.savedRequest?.kind ?? this.downloadKind));
      if (invalid.length) {
        this.statusText = `以下任务缺少有效手动选流，请先解析并选择，或切换自动模式：${invalid.map(item => item.title).join("、")}`;
        return;
      }
    }
    const template = this.buildOutputTemplate();

    this.isBusy = true;
    this.progress = 0;
    this.isProgressVisible = true;
    this.speedText = "";
    this.etaText = "";
    this.logs = [];
    this.startTaskbar();

    this.queueTotal = targets.length;
    this.runTargetIds = targets.map(item => item.id);
    this.completedTargetIds.clear();
    let completed = 0;
    let failed = 0;
    let cancelled = false;

    try {
      const requests = targets.map((item, index) => {
        const request = item.savedRequest ? { ...item.savedRequest, cookieFile: this.selectedCookiePath } : this.buildRequest(
          item.url, item.selectedVideo, item.selectedAudio, this.isManualMode,
          this.isFixedNameMode && targets.length > 1 ? `${this.fixedFileName.trim()} - ${String(index + 1).padStart(3, "0")}.${EXTENSION_TOKEN}` : template,
        );
        if (!request.auto) {
        request.manual = { videoFormatId: request.kind === "AudioOnly" ? null : item.selectedVideo?.formatId ?? null, videoHasAudio: item.selectedVideo?.hasAudio ?? false, audioFormatId: request.kind === "VideoOnly" || item.selectedVideo?.hasAudio ? null : item.selectedAudio?.formatId ?? null };
        }
        request.live = item.isLive;
        item.savedRequest = request;
        return { itemId: item.id, request };
      });
      await this.queueSaveChain;
      await api.saveQueue(this.queue);
      if (this.stopRequested) throw {kind: "cancelled"};
      const outcome = await api.startQueue(requests);
      const persisted = await api.loadQueue() as QueueItem[];
      for (const saved of persisted) {
        const local = this.queue.find(item => item.id === saved.id);
        if (local) Object.assign(local, saved);
      }
      completed = outcome.completed; failed = outcome.failed; cancelled = outcome.cancelled;
      this.statusText = cancelled ? "队列已停止，剩余任务已保留。" : `队列下载完成：成功 ${completed} 个，失败 ${failed} 个。`;
      if (!cancelled) void api.notify("队列下载完成", this.statusText);
    } catch (caught) {
      cancelled = isCancelled(caught);
      this.statusText = cancelled ? "队列下载已取消" : `下载失败：${errorMessage(caught)}`;
    } finally {
      this.isProgressVisible = false;
      this.queueProgressText = "";
      this.queueTotal = 0;
      this.currentItemId = null;
      this.isBusy = false;
      this.activeJobId = null;
      await this.persistQueue();
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
      saveLog: this.saveLog,
      cookieFile: this.selectedCookiePath,
      url,
      auto: !manual,
      kind: this.downloadKind,
      resolution: this.resolution,
      manual: manualPayload && { ...manualPayload,
        videoFormatId: this.downloadKind === "AudioOnly" ? null : manualPayload.videoFormatId,
        audioFormatId: this.downloadKind === "VideoOnly" || manualPayload.videoHasAudio ? null : manualPayload.audioFormatId,
      },
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
    if (!linkKey(url)) { this.statusText = "请输入有效的 HTTP / HTTPS 链接"; return; }
    if (this.queue.some(item => linkKey(item.url) === linkKey(url))) {
      this.statusText = "该链接已在队列中；如需保留重复项，请使用批量导入。"; return;
    }

    this.queue.push(this.makeItem(url));
    void this.persistQueue();
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
    void this.persistQueue();
  }

  removeQueueItem(id: number): void {
    if (this.isBusy) return;
    this.queue = this.queue.filter((item) => item.id !== id);
    if (this.selectedQueueItemId === id) this.selectedQueueItemId = null;
    if (this.queue.length === 0) this.statusText = "队列已空。";
    void this.persistQueue();
  }

  // ------------------------------------------------------------- progress

  /** Handles one `download:update` event from the backend. */
  handleUpdate(update: DownloadUpdateDto): void {
    if (!this.isBusy || update.jobId < this.lastJobId || (this.activeJobId !== null && this.activeJobId !== update.jobId)) return;
    this.lastJobId = update.jobId;
    this.activeJobId = update.jobId;
    if (update.itemId !== null) this.currentItemId = update.itemId;
    if (update.phase) {
      const phase = update.phase.split(":")[0];
      this.statusText = ({DownloadingVideo: "正在下载视频", DownloadingAudio: "正在下载音频", Processing: "正在合并 / 后处理", Completed: "文件已保存"} as Record<string,string>)[phase] ?? "正在下载";
      if (update.elapsed !== null && update.downloadedBytes !== null) this.statusText += ` · ${Math.round(update.elapsed)} 秒 · ${(update.downloadedBytes / 1048576).toFixed(1)} MiB`;
    }
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
        this.progress = ((this.completedTargetIds.size + itemPercent / 100) / this.queueTotal) * 100;
      }
    }

    this.updateTaskbar();
  }

  cancel(): void {
    this.stopRequested = true;
    this.statusText = "正在取消…";
    void api.cancelJob(this.activeJobId);
  }

  handleQueueUpdate(update: {jobId: number; itemId: number; patch: Partial<QueueItem>}): void {
    if (!this.isBusy || update.jobId < this.lastJobId || (this.activeJobId !== null && this.activeJobId !== update.jobId)) return;
    this.activeJobId = update.jobId; this.lastJobId = update.jobId;
    const item = this.queue.find(item => item.id === update.itemId);
    if (item) Object.assign(item, update.patch);
    if (update.patch.state === "Downloading") {
      this.currentItemId = update.itemId;
      this.queueIndex = this.runTargetIds.indexOf(update.itemId);
      this.queueProgressText = `下载 ${this.queueIndex + 1} / ${this.queueTotal}`;
      this.speedText = ""; this.etaText = "";
    }
    if (update.patch.state === "Completed" || update.patch.state === "Failed") {
      this.completedTargetIds.add(update.itemId);
      if (update.patch.outputPath) this.lastOutputPath = update.patch.outputPath;
    }
  }

  async persistQueue(): Promise<void> {
    if (!this.queueLoaded) return;
    const snapshot = JSON.parse(JSON.stringify(this.queue)) as QueueItem[];
    for (const item of snapshot) if (item.savedRequest) item.savedRequest.cookieFile = null;
    const save = this.queueSaveChain.then(() => api.saveQueue(snapshot));
    this.queueSaveChain = save.catch(error => { this.statusText = errorMessage(error); });
    await this.queueSaveChain;
  }

  async retryItems(ids: number[]): Promise<void> {
    if (this.isBusy) return;
    await settingsStore.flush();
    if (settingsStore.error) { this.statusText = settingsStore.error; return; }
    this.stopRequested = false;
    for (const item of this.queue.filter(item => ids.includes(item.id))) {
      if (item.state === "Completed") item.state = "Pending";
    }
    await this.downloadQueue(ids);
  }

  async copyDiagnostics(): Promise<void> {
    const sensitive = [this.selectedCookiePath, settingsStore.value.CookieFile].filter(Boolean) as string[];
    let text = [this.statusText, ...this.queue.filter(item => item.error).map(item => `${item.title}: ${item.error}`), ...this.logs].join("\n");
    for (const value of sensitive) text = text.split(value).join("[Cookie 文件已隐藏]");
    text = text.replace(/(cookie|authorization|proxy-authorization)\s*[:=]\s*[^\r\n]+/gi, "$1: [已隐藏]").replace(/:\/\/[^/@\s]+:[^/@\s]+@/g, "://[已隐藏]@");
    try { await navigator.clipboard.writeText(text); this.statusText = "诊断信息已复制"; }
    catch (error) { this.statusText = `复制失败：${errorMessage(error)}`; }
  }

  async reveal(path: string | null): Promise<void> {
    if (!path) return;
    try { await api.revealPath(path); }
    catch (error) { this.statusText = errorMessage(error); }
  }

  async openDirectory(): Promise<void> {
    try { await api.openDirectory(this.downloadDirectory.trim() || this.defaultDirectory); }
    catch (error) { this.statusText = errorMessage(error); }
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
