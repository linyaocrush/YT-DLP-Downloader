// Settings-page detection state and actions (mirrors `SettingsViewModel`).

import * as api from "./api";
import { settingsStore, ui } from "./store.svelte";
import {
  FFMPEG_DOWNLOAD_URL,
  YTDLP_DOWNLOAD_URL,
  type AppTheme,
  type CommandError,
  type ProxyListMode,
  type ProxyProtocol,
} from "./types";

function errorMessage(error: unknown): string {
  const typed = error as CommandError | undefined;
  return typed?.message ?? String(error);
}

function sameText(a: string, b: string): boolean {
  return a.toLowerCase() === b.toLowerCase();
}

class SettingsPageStore {
  statusText = $state("");
  showYtDlpDownloadHint = $state(false);
  ytDlpReady = $state(false);

  ffmpegStatus = $state("");
  showFfmpegManualControls = $state(false);
  showFfmpegDownloadHint = $state(false);
  ffmpegReady = $state(false);

  cookieFolderStatus = $state("");
  proxyStatus = $state("");
  newProxySite = $state("");

  get isEnvironmentReady(): boolean {
    return this.ytDlpReady && this.ffmpegReady;
  }

  get isEnvironmentPartial(): boolean {
    return this.ytDlpReady !== this.ffmpegReady;
  }

  get isEnvironmentMissing(): boolean {
    return !this.ytDlpReady && !this.ffmpegReady;
  }

  get environmentStatusText(): string {
    const yt = this.ytDlpReady;
    const ff = this.ffmpegReady;
    if (yt && ff) return "运行环境正常：已检测到 yt-dlp 与 ffmpeg。";
    if (yt && !ff) return "部分就绪：已检测到 yt-dlp，但缺少可用的 ffmpeg。";
    if (!yt && ff) return "部分就绪：已检测到 ffmpeg，但缺少可用的 yt-dlp。";
    return "环境未就绪：未检测到可用的 yt-dlp 与 ffmpeg。";
  }

  async initialize(): Promise<void> {
    await Promise.all([
      this.refreshDetection(),
      this.refreshFfmpegStatus(),
      this.refreshCookieFolderStatus(),
      this.refreshProxyStatus(),
    ]);
  }

  // --- yt-dlp -------------------------------------------------------------

  async refreshDetection(): Promise<void> {
    this.statusText = "正在检测 yt-dlp…";
    try {
      const detection = await api.detectYtDlp();
      this.showYtDlpDownloadHint = detection.showDownloadHint;
      this.ytDlpReady = detection.ready;
      this.statusText = detection.statusText;
      if (detection.resolved && !sameText(detection.resolved, settingsStore.value.YtDlpPath)) {
        settingsStore.setLocal("YtDlpPath", detection.resolved);
      }
    } catch (error) {
      this.ytDlpReady = false;
      this.statusText = `检测失败：${errorMessage(error)}`;
    }
  }

  async browseYtDlp(): Promise<void> {
    const picked = await api.pickFile("选择 yt-dlp 可执行文件", [
      ["yt-dlp 可执行文件 (*.exe)", ["exe"]],
      ["所有文件 (*.*)", ["*"]],
    ]);
    if (!picked) return;
    settingsStore.set("YtDlpPath", picked);
    await this.refreshDetection();
  }

  async setYtDlpPath(value: string): Promise<void> {
    settingsStore.setLocal("YtDlpPath", value);
    await settingsStore.persist({ YtDlpPath: value.trim() });
  }

  async rescan(): Promise<void> {
    await this.refreshDetection();
    await this.refreshFfmpegStatus();
  }

  // --- ffmpeg -------------------------------------------------------------

  async refreshFfmpegStatus(): Promise<void> {
    const status = await api.ffmpegStatus();
    this.showFfmpegManualControls = status.showManualControls;
    this.showFfmpegDownloadHint = status.showDownloadHint;
    this.ffmpegReady = status.ready;
    this.ffmpegStatus = status.statusText;
  }

  async setFfmpegPath(value: string): Promise<void> {
    settingsStore.setLocal("FfmpegPath", value);
    await settingsStore.persist({ FfmpegPath: value.trim() });
    await this.refreshFfmpegStatus();
  }

  async browseFfmpeg(): Promise<void> {
    const picked = await api.pickFile("选择 ffmpeg 可执行文件", [
      ["ffmpeg 可执行文件 (ffmpeg.exe)", ["exe"]],
      ["所有文件 (*.*)", ["*"]],
    ]);
    if (!picked) return;
    await this.setFfmpegPath(picked);
  }

  async clearFfmpeg(): Promise<void> {
    await this.setFfmpegPath("");
  }

  // --- cookies ------------------------------------------------------------

  async refreshCookieFolderStatus(): Promise<void> {
    this.cookieFolderStatus = await api.cookieFolderStatus();
  }

  async setCookieFolder(value: string): Promise<void> {
    settingsStore.setLocal("CookieFolder", value);
    await settingsStore.persist({ CookieFolder: value.trim() });
    await this.refreshCookieFolderStatus();
  }

  async browseCookieFolder(): Promise<void> {
    const picked = await api.pickFolder("选择存放 Cookie 文件的文件夹");
    if (!picked) return;
    await this.setCookieFolder(picked);
  }

  async clearCookieFolder(): Promise<void> {
    await this.setCookieFolder("");
  }

  // --- proxy --------------------------------------------------------------

  async refreshProxyStatus(): Promise<void> {
    this.proxyStatus = await api.proxySummary();
  }

  async setProxyEnabled(value: boolean): Promise<void> {
    await settingsStore.set("ProxyEnabled", value);
    await this.refreshProxyStatus();
  }

  async setProxyProtocol(value: ProxyProtocol): Promise<void> {
    await settingsStore.set("ProxyProtocol", value);
    await this.refreshProxyStatus();
  }

  async setProxyHost(value: string): Promise<void> {
    settingsStore.setLocal("ProxyHost", value);
    await settingsStore.persist({ ProxyHost: value.trim() });
    await this.refreshProxyStatus();
  }

  async setProxyPort(value: string): Promise<void> {
    settingsStore.setLocal("ProxyPort", value);
    await settingsStore.persist({ ProxyPort: value.trim() });
    await this.refreshProxyStatus();
  }

  async setProxyListMode(mode: ProxyListMode): Promise<void> {
    await settingsStore.set("ProxyListMode", mode);
    await this.refreshProxyStatus();
  }

  async addProxySite(): Promise<void> {
    const site = this.newProxySite.trim();
    if (!site) return;

    const sites = settingsStore.value.ProxySites;
    if (!sites.some((entry) => sameText(entry, site))) {
      await settingsStore.set("ProxySites", [...sites, site]);
    }
    this.newProxySite = "";
    await this.refreshProxyStatus();
  }

  async removeProxySite(site: string): Promise<void> {
    await settingsStore.set(
      "ProxySites",
      settingsStore.value.ProxySites.filter((entry) => entry !== site),
    );
    await this.refreshProxyStatus();
  }

  // --- theme / links ------------------------------------------------------

  setTheme(theme: AppTheme): void {
    settingsStore.set("Theme", theme);
    ui.applyTheme(theme);
  }

  openYtDlpDownload(): void {
    void api.openUrl(YTDLP_DOWNLOAD_URL);
  }

  openFfmpegDownload(): void {
    void api.openUrl(FFMPEG_DOWNLOAD_URL);
  }
}

export const settingsPage = new SettingsPageStore();