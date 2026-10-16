// Types mirroring the Rust DTOs in `src-tauri/src/dto.rs` and the persisted
// `AppSettings` (which keeps the C# PascalCase JSON field names on purpose).

export type AppTheme = "Light" | "Dark" | "System";
export type DownloadKind = "Default" | "VideoOnly" | "AudioOnly";
export type OutputNameMode = "Template" | "Fixed";
export type ProxyProtocol = "Http" | "Https" | "Socks5";
export type ProxyListMode = "Whitelist" | "Blacklist";

export type ResolutionPreference =
  | "Best"
  | "Above4K"
  | "UpTo4K"
  | "UpTo2K"
  | "UpTo1080P"
  | "UpTo720P"
  | "UpTo360P";

/** Persisted user settings. Field names deliberately match `settings.json`. */
export interface AppSettings {
  YtDlpPath: string;
  FfmpegPath: string;
  DownloadDirectory: string;
  CookieFile: string;
  CookieFolder: string;
  DownloadThreads: number;
  OutputNameMode: OutputNameMode;
  OutputTemplate: string;
  OutputFileName: string;
  DownloadKind: DownloadKind;
  Resolution: ResolutionPreference;
  WriteThumbnail: boolean;
  ProxyEnabled: boolean;
  ProxyProtocol: ProxyProtocol;
  ProxyHost: string;
  ProxyPort: string;
  ProxyListMode: ProxyListMode;
  ProxySites: string[];
  Theme: AppTheme;
}

export interface VideoSourceDto {
  formatId: string;
  resolution: string;
  fpsText: string;
  bitrateText: string;
  codec: string;
  kind: string;
  hasAudio: boolean;
  container: string;
}

export interface AudioSourceDto {
  formatId: string;
  bitrateText: string;
  codec: string;
  container: string;
}

export interface MediaInfoDto {
  id: string;
  title: string;
  webpageUrl: string;
  durationText: string;
  uploader: string | null;
  thumbnailUrl: string | null;
  videos: VideoSourceDto[];
  audios: AudioSourceDto[];
  isLive: boolean;
  isUpcoming: boolean;
}

export interface PlaylistEntryDto {
  url: string;
  title: string;
}

export interface PlaylistInfoDto {
  id: string;
  title: string;
  entries: PlaylistEntryDto[];
}

export interface ParseResultDto {
  media: MediaInfoDto | null;
  playlist: PlaylistInfoDto | null;
}

export interface DownloadUpdateDto {
  line: string | null;
  percent: number | null;
  outputPath: string | null;
  speed: string | null;
  eta: string | null;
}

export interface DownloadOutcomeDto {
  success: boolean;
  cancelled: boolean;
  outputPath: string | null;
  error: string | null;
}

export interface ManualSelectionDto {
  videoFormatId: string | null;
  videoHasAudio: boolean;
  audioFormatId: string | null;
}

export interface DownloadRequestDto {
  url: string;
  auto: boolean;
  kind: DownloadKind;
  resolution: ResolutionPreference;
  manual: ManualSelectionDto | null;
  outputDirectory: string;
  outputTemplate: string | null;
  writeThumbnail: boolean;
  live: boolean;
}

export interface YtDlpStatusDto {
  path: string | null;
  ready: boolean;
  text: string;
}

export interface YtDlpDetectionDto {
  resolved: string | null;
  version: string | null;
  ready: boolean;
  showDownloadHint: boolean;
  statusText: string;
}

export interface FfmpegStatusDto {
  locatedInPath: string | null;
  configuredPath: string;
  configuredExists: boolean;
  ready: boolean;
  showManualControls: boolean;
  showDownloadHint: boolean;
  statusText: string;
}

export interface CookieFileDto {
  path: string;
  name: string;
}

export interface CookieListingDto {
  folder: string;
  exists: boolean;
  needsSetup: boolean;
  hint: string;
  files: CookieFileDto[];
}

export interface ProxyIndicatorDto {
  willApply: boolean;
  text: string;
  detail: string;
  proxyUrl: string | null;
}

/** Rejection value produced by the Rust `CommandError`. */
export interface CommandError {
  kind: "cancelled" | "message" | "io";
  message: string;
}

// ---------------------------------------------------------------------------
// UI-only catalogs (mirror the C# static catalogs)
// ---------------------------------------------------------------------------

export interface ResolutionOption {
  value: ResolutionPreference;
  label: string;
}

export const RESOLUTION_CATALOG: ResolutionOption[] = [
  { value: "Best", label: "最佳" },
  { value: "Above4K", label: "> 4K" },
  { value: "UpTo4K", label: "≤ 4K" },
  { value: "UpTo2K", label: "≤ 2K" },
  { value: "UpTo1080P", label: "≤ 1080P" },
  { value: "UpTo720P", label: "≤ 720P" },
  { value: "UpTo360P", label: "≤ 360P" },
];

export interface TemplateField {
  token: string;
  label: string;
  selected: boolean;
}

export const TEMPLATE_FIELD_CATALOG: ReadonlyArray<{ token: string; label: string }> = [
  { token: "%(title)s", label: "标题" },
  { token: "%(id)s", label: "视频 ID" },
  { token: "%(uploader)s", label: "上传者" },
  { token: "%(upload_date)s", label: "上传日期" },
  { token: "%(resolution)s", label: "分辨率" },
  { token: "%(fps)s", label: "帧率" },
  { token: "%(duration)s", label: "时长" },
  { token: "%(extractor)s", label: "站点" },
  { token: "%(format_id)s", label: "格式 ID" },
  { token: "%(vcodec)s", label: "视频编码" },
  { token: "%(acodec)s", label: "音频编码" },
];

export const EXTENSION_TOKEN = "%(ext)s";

export const YTDLP_DOWNLOAD_URL = "https://github.com/yt-dlp/yt-dlp/releases";
export const FFMPEG_DOWNLOAD_URL = "https://ffmpeg.org/download.html";

export function defaultSettings(): AppSettings {
  return {
    YtDlpPath: "",
    FfmpegPath: "",
    DownloadDirectory: "",
    CookieFile: "",
    CookieFolder: "",
    DownloadThreads: 4,
    OutputNameMode: "Template",
    OutputTemplate: "%(title)s",
    OutputFileName: "",
    DownloadKind: "Default",
    Resolution: "Best",
    WriteThumbnail: false,
    ProxyEnabled: false,
    ProxyProtocol: "Http",
    ProxyHost: "",
    ProxyPort: "",
    ProxyListMode: "Whitelist",
    ProxySites: [],
    Theme: "System",
  };
}