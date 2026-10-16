// Typed wrappers around the Rust commands. Everything the UI can do goes
// through here, so the command surface stays in one place.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppSettings,
  AppTheme,
  CookieListingDto,
  DownloadOutcomeDto,
  DownloadRequestDto,
  DownloadUpdateDto,
  FfmpegStatusDto,
  MediaInfoDto,
  ParseResultDto,
  ProxyIndicatorDto,
  YtDlpDetectionDto,
  YtDlpStatusDto,
} from "./types";

export const DOWNLOAD_UPDATE_EVENT = "download:update";

export const getSettings = () => invoke<AppSettings>("get_settings");

export const updateSettings = (patch: Partial<AppSettings>) =>
  invoke<AppSettings>("update_settings", { patch });

export const ytdlpStatus = () => invoke<YtDlpStatusDto>("ytdlp_status");

export const detectYtDlp = () => invoke<YtDlpDetectionDto>("detect_ytdlp");

export const ffmpegStatus = () => invoke<FfmpegStatusDto>("ffmpeg_status");

export const defaultDownloadDir = () => invoke<string>("default_download_dir");

export const listCookieFiles = () => invoke<CookieListingDto>("list_cookie_files");

export const cookieFolderStatus = () => invoke<string>("cookie_folder_status");

export const proxyIndicator = (url: string | null) =>
  invoke<ProxyIndicatorDto>("proxy_indicator", { url });

export const proxySummary = () => invoke<string>("proxy_summary");

export const parseMedia = (url: string) => invoke<ParseResultDto>("parse_media", { url });

export const getMediaInfo = (url: string) => invoke<MediaInfoDto>("get_media_info", { url });

export const startDownload = (request: DownloadRequestDto) =>
  invoke<DownloadOutcomeDto>("start_download", { request });

export const cancelJob = () => invoke<void>("cancel_job");

export const pickFile = (title: string, filters: Array<[string, string[]]>) =>
  invoke<string | null>("pick_file", { title, filters });

export const pickFolder = (title: string) => invoke<string | null>("pick_folder", { title });

export const openUrl = (url: string) => invoke<void>("open_url", { url });

export const revealPath = (path: string) => invoke<void>("reveal_path", { path });

export const notify = (title: string, body: string) =>
  invoke<void>("notify", { title, body });

export type TaskbarStatus = "none" | "normal" | "error" | "indeterminate" | "paused";

export const setTaskbarProgress = (status: TaskbarStatus, progress: number | null) =>
  invoke<void>("set_taskbar_progress", { status, progress });

export const setWindowTheme = (theme: AppTheme) =>
  invoke<void>("set_window_theme", { theme });

export const onDownloadUpdate = (handler: (update: DownloadUpdateDto) => void): Promise<UnlistenFn> =>
  listen<DownloadUpdateDto>(DOWNLOAD_UPDATE_EVENT, (event) => handler(event.payload));