//! Tauri commands. Each one is a thin shell over the ported core services; the
//! Svelte frontend owns the UI state and orchestration.

use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use tauri::window::{ProgressBarState, ProgressBarStatus};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

use ytdlp_downloader_core::{
    AppSettings, CancellationToken, DownloadKind, DownloadRequest, DownloadUpdate,
    ProgressCallback, ProxyListMode, ProxyRules, YtDlpFormat,
};

use crate::dto::{
    CookieFileDto, CookieListingDto, DownloadOutcomeDto, DownloadRequestDto, DownloadUpdateDto,
    FfmpegStatusDto, MediaInfoDto, ParseResultDto, PlaylistInfoDto, ProxyIndicatorDto,
    YtDlpDetectionDto, YtDlpStatusDto,
};
use crate::error::CommandError;
use crate::state::AppState;
use crate::util;

/// Event carrying incremental download progress to the frontend.
pub const DOWNLOAD_UPDATE_EVENT: &str = "download:update";

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.settings()
}

/// Shallow-merges `patch` into the stored settings and persists the result. The
/// frontend only sends the fields it changed.
#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: Value,
) -> Result<AppSettings, CommandError> {
    let mut base = serde_json::to_value(state.settings.settings())
        .map_err(|error| CommandError::io(error.to_string()))?;
    merge(&mut base, patch);

    let updated: AppSettings =
        serde_json::from_value(base).map_err(|error| CommandError::message(error.to_string()))?;

    state
        .settings
        .update(&mut |settings| *settings = updated.clone());
    state.settings.save();
    Ok(updated)
}

// ---------------------------------------------------------------------------
// Environment detection
// ---------------------------------------------------------------------------

/// Cheap status for the download page: only resolves the executable.
#[tauri::command]
pub fn ytdlp_status(state: State<'_, AppState>) -> YtDlpStatusDto {
    let path = state.cli.resolve_executable();
    let text = if path.is_none() {
        "未检测到 yt-dlp，请到“设置”页配置".to_string()
    } else {
        "yt-dlp 已就绪".to_string()
    };

    YtDlpStatusDto {
        ready: path.is_some(),
        path,
        text,
    }
}

/// Full detection for the settings page: resolves the path and runs `--version`.
#[tauri::command]
pub async fn detect_ytdlp(app: AppHandle) -> YtDlpDetectionDto {
    let (settings_service, paths, cli) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.settings),
            Arc::clone(&state.paths),
            Arc::clone(&state.cli),
        )
    };

    let configured = settings_service.settings().yt_dlp_path.trim().to_string();

    let Some(resolved) = paths.resolve() else {
        let status_text = if configured.is_empty() {
            "未在 PATH 中找到 yt-dlp。请点击“浏览…”选择 yt-dlp.exe，或直接输入完整路径。"
                .to_string()
        } else {
            "未检测到可用的 yt-dlp，请检查路径是否正确。".to_string()
        };

        return YtDlpDetectionDto {
            resolved: None,
            version: None,
            ready: false,
            show_download_hint: true,
            status_text,
        };
    };

    let version = cli
        .get_version(CancellationToken::new())
        .await
        .ok()
        .flatten();
    let ready = version.is_some();
    let status_text = match &version {
        Some(version) => format!("已就绪 — yt-dlp {version}"),
        None => format!("无法运行该程序，请确认它是有效的 yt-dlp：{resolved}"),
    };

    YtDlpDetectionDto {
        resolved: Some(resolved),
        version,
        ready,
        show_download_hint: false,
        status_text,
    }
}

#[tauri::command]
pub fn ffmpeg_status(state: State<'_, AppState>) -> FfmpegStatusDto {
    let configured = state.settings.settings().ffmpeg_path.trim().to_string();
    let configured_exists = !configured.is_empty() && Path::new(&configured).is_file();

    if let Some(located) = state.ffmpeg_paths.locate_in_path() {
        return FfmpegStatusDto {
            status_text: format!("已在系统 PATH 中检测到 ffmpeg：{located}（无需手动配置）"),
            located_in_path: Some(located),
            configured_path: configured,
            configured_exists,
            ready: true,
            show_manual_controls: false,
            show_download_hint: false,
        };
    }

    let status_text = if configured.is_empty() {
        "未在系统 PATH 中检测到 ffmpeg。合成音视频需要它，请点击“浏览…”手动指定 ffmpeg.exe 的路径。"
            .to_string()
    } else if configured_exists {
        format!("已配置 ffmpeg：{configured}")
    } else {
        "指定的 ffmpeg 路径不存在，请重新选择。".to_string()
    };

    FfmpegStatusDto {
        located_in_path: None,
        configured_path: configured,
        configured_exists,
        ready: configured_exists,
        show_manual_controls: true,
        show_download_hint: state.ffmpeg_paths.configured_path().is_none(),
        status_text,
    }
}

#[tauri::command]
pub fn default_download_dir() -> String {
    util::default_download_directory()
}

// ---------------------------------------------------------------------------
// Cookies
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_cookie_files(state: State<'_, AppState>) -> CookieListingDto {
    let folder = state.settings.settings().cookie_folder.trim().to_string();
    let exists = !folder.is_empty() && Path::new(&folder).is_dir();

    let mut files = Vec::new();
    if exists {
        if let Ok(entries) = std::fs::read_dir(&folder) {
            files = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_file())
                .map(|entry| {
                    let path = entry.path();
                    CookieFileDto {
                        name: path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        path: path.to_string_lossy().into_owned(),
                    }
                })
                .collect();
            files.sort_by_key(|file| file.name.to_lowercase());
        }
    }

    let hint = if folder.is_empty() {
        "未设置 Cookie 文件夹。请到“设置”页选择一个文件夹，即可在此选择其中的 Cookie 文件。"
            .to_string()
    } else if exists {
        "可选：用于需要登录或年龄限制的视频，从上方下拉列表选择 Cookie 文件。".to_string()
    } else {
        "Cookie 文件夹不存在，请到“设置”页重新选择。".to_string()
    };

    CookieListingDto {
        folder,
        exists,
        needs_setup: !exists,
        hint,
        files,
    }
}

#[tauri::command]
pub fn cookie_folder_status(state: State<'_, AppState>) -> String {
    let folder = state.settings.settings().cookie_folder.trim().to_string();

    if folder.is_empty() {
        return "未设置：下载页将不使用 Cookie。".to_string();
    }
    if !Path::new(&folder).is_dir() {
        return "文件夹不存在，请重新选择。".to_string();
    }

    match std::fs::read_dir(&folder) {
        Err(error) => format!("无法读取文件夹：{error}"),
        Ok(entries) => {
            let count = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_file())
                .count();
            if count == 0 {
                "该文件夹内未找到 Cookie 文件。".to_string()
            } else {
                format!("已识别 {count} 个 Cookie 文件，可在下载页的下拉列表中选择。")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Proxy status
// ---------------------------------------------------------------------------

/// Short indicator shown on the download page (depends on the current URL).
#[tauri::command]
pub fn proxy_indicator(state: State<'_, AppState>, url: Option<String>) -> ProxyIndicatorDto {
    let settings = state.settings.settings();
    let proxy_url = ProxyRules::build_proxy_url(&settings);
    let will_apply = ProxyRules::should_use_proxy(&settings, url.as_deref());
    let configured = proxy_url.as_deref().unwrap_or_default();

    let (text, detail) = if proxy_url.is_none() {
        if settings.proxy_enabled {
            (
                "代理：未配置完整".to_string(),
                "已在设置中开启代理，但 IP 或端口无效，本次不会使用代理。".to_string(),
            )
        } else {
            (
                "代理：未启用".to_string(),
                "代理总开关已关闭，本次下载与解析都不会使用代理。".to_string(),
            )
        }
    } else if will_apply {
        (
            "代理：将使用".to_string(),
            format!("本次下载将通过 {configured} 访问网络。"),
        )
    } else {
        (
            "代理：不使用".to_string(),
            format!("当前链接不在代理规则范围内，本次将直连（代理配置 {configured}）。"),
        )
    };

    ProxyIndicatorDto {
        will_apply,
        text,
        detail,
        proxy_url,
    }
}

/// Rule summary shown on the settings page.
#[tauri::command]
pub fn proxy_summary(state: State<'_, AppState>) -> String {
    let settings = state.settings.settings();

    if !settings.proxy_enabled {
        return "代理总开关已关闭：下载与解析都不会添加代理参数。".to_string();
    }

    let Some(proxy_url) = ProxyRules::build_proxy_url(&settings) else {
        return "请在下方填写有效的 IP / 主机与端口（1-65535）。".to_string();
    };

    let count = settings
        .proxy_sites
        .iter()
        .filter(|site| !site.trim().is_empty())
        .count();

    let scope = match settings.proxy_list_mode {
        ProxyListMode::Whitelist => {
            if count == 0 {
                "白名单为空：不会对任何网站使用代理。".to_string()
            } else {
                format!("白名单模式：仅对名单中的 {count} 个网站使用代理。")
            }
        }
        ProxyListMode::Blacklist => {
            if count == 0 {
                "黑名单为空：所有网站都会使用代理。".to_string()
            } else {
                format!("黑名单模式：对名单外的所有网站使用代理（已排除 {count} 个网站）。")
            }
        }
    };

    format!("代理已启用：{proxy_url}，{scope}")
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parses a URL that may be a single video or a playlist.
#[tauri::command]
pub async fn parse_media(app: AppHandle, url: String) -> Result<ParseResultDto, CommandError> {
    let (cli, settings_service, cancellation) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.cli),
            Arc::clone(&state.settings),
            state.begin_job(),
        )
    };

    let settings = settings_service.settings();
    let cookie = util::cookie_argument(&settings);
    let proxy = util::proxy_argument(&settings, &url);

    let result = cli
        .get_media_or_playlist(&url, cookie.as_deref(), proxy.as_deref(), cancellation)
        .await;

    app.state::<AppState>().end_job();

    let parsed = result?;
    Ok(ParseResultDto {
        media: parsed.media.as_ref().map(MediaInfoDto::from),
        playlist: parsed.playlist.as_ref().map(PlaylistInfoDto::from),
    })
}

/// Parses one concrete video (used for each queue item). Unlike [`parse_media`]
/// this never expands a playlist.
#[tauri::command]
pub async fn get_media_info(app: AppHandle, url: String) -> Result<MediaInfoDto, CommandError> {
    let (cli, settings_service, cancellation) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.cli),
            Arc::clone(&state.settings),
            state.begin_job(),
        )
    };

    let settings = settings_service.settings();
    let cookie = util::cookie_argument(&settings);
    let proxy = util::proxy_argument(&settings, &url);

    let result = cli
        .get_media_info(&url, cookie.as_deref(), proxy.as_deref(), cancellation)
        .await;

    app.state::<AppState>().end_job();

    Ok(MediaInfoDto::from(&result?))
}

// ---------------------------------------------------------------------------
// Downloads
// ---------------------------------------------------------------------------

/// Runs one download to completion, emitting [`DOWNLOAD_UPDATE_EVENT`] as it
/// progresses. The promise resolves with the final outcome.
#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    request: DownloadRequestDto,
) -> Result<DownloadOutcomeDto, CommandError> {
    // Build the format expression before claiming the job slot so a validation
    // error cannot leave a stale token behind.
    let expression = build_expression(&request)?;

    let (cli, settings_service, cancellation) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.cli),
            Arc::clone(&state.settings),
            state.begin_job(),
        )
    };

    let settings = settings_service.settings();
    let directory = if request.output_directory.trim().is_empty() {
        util::default_download_directory()
    } else {
        request.output_directory.trim().to_string()
    };

    let mut download = DownloadRequest::new(request.url.clone(), expression, directory);
    download.cookie_file = util::cookie_argument(&settings);
    download.proxy_url = util::proxy_argument(&settings, &request.url);
    download.concurrent_fragments = if settings.download_threads >= 1 {
        settings.download_threads
    } else {
        4
    };
    download.output_template = request.output_template.clone();
    download.write_thumbnail = request.write_thumbnail;
    download.live = request.live;

    let emitter = app.clone();
    let progress: ProgressCallback = Arc::new(move |update: DownloadUpdate| {
        let _ = emitter.emit(DOWNLOAD_UPDATE_EVENT, DownloadUpdateDto::from(update));
    });

    let result = cli.download(download, Some(progress), cancellation).await;

    app.state::<AppState>().end_job();

    let outcome = result?;
    Ok(DownloadOutcomeDto {
        success: outcome.success,
        cancelled: outcome.cancelled,
        output_path: outcome.output_path,
        error: outcome.error,
    })
}

#[tauri::command]
pub fn cancel_job(app: AppHandle) {
    app.state::<AppState>().cancel_job();
}

/// Builds the yt-dlp `-f` value. Auto mode delegates to the core's
/// `YtDlpFormat`; manual mode mirrors `DownloadViewModel.BuildFormatExpression`.
fn build_expression(request: &DownloadRequestDto) -> Result<String, CommandError> {
    if request.auto {
        return Ok(YtDlpFormat::build_expression(
            request.kind,
            request.resolution,
        ));
    }

    let video = || {
        request
            .manual
            .as_ref()
            .and_then(|manual| manual.video_format_id.clone())
            .ok_or_else(|| CommandError::message("请先选择视频源"))
    };
    let audio = || {
        request
            .manual
            .as_ref()
            .and_then(|manual| manual.audio_format_id.clone())
            .ok_or_else(|| CommandError::message("请先选择音频源"))
    };

    match request.kind {
        DownloadKind::VideoOnly => video(),
        DownloadKind::AudioOnly => audio(),
        DownloadKind::Default => {
            let video = video()?;
            if request
                .manual
                .as_ref()
                .is_some_and(|manual| manual.video_has_audio)
            {
                Ok(video)
            } else {
                Ok(format!("{video}+{}", audio()?))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shell integration
// ---------------------------------------------------------------------------

/// Native file picker. Filters are `(name, extensions)` pairs.
#[tauri::command]
pub async fn pick_file(
    app: AppHandle,
    title: String,
    filters: Vec<(String, Vec<String>)>,
) -> Result<Option<String>, CommandError> {
    let mut builder = app.dialog().file().set_title(title);
    for (name, extensions) in filters {
        let refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
        if !refs.is_empty() {
            builder = builder.add_filter(name, &refs);
        }
    }

    let (sender, receiver) = tokio::sync::oneshot::channel();
    builder.pick_file(move |path| {
        let _ = sender.send(path);
    });

    let picked = receiver
        .await
        .map_err(|error| CommandError::io(error.to_string()))?;
    Ok(picked.and_then(file_path_to_string))
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle, title: String) -> Result<Option<String>, CommandError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .pick_folder(move |path| {
            let _ = sender.send(path);
        });

    let picked = receiver
        .await
        .map_err(|error| CommandError::io(error.to_string()))?;
    Ok(picked.and_then(file_path_to_string))
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), CommandError> {
    app.opener()
        .open_url(url, None::<String>)
        .map_err(|error| CommandError::io(error.to_string()))
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> Result<(), CommandError> {
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|error| CommandError::io(error.to_string()))
}

/// Windows toast notification (stand-in for the WPF tray balloon).
#[tauri::command]
pub fn notify(app: AppHandle, title: String, body: String) -> Result<(), CommandError> {
    app.notification()
        .builder()
        .title(title)
        .body(util::truncate(&body, 250))
        .show()
        .map_err(|error| CommandError::io(error.to_string()))
}

/// Mirrors download progress onto the Windows taskbar button.
#[tauri::command]
pub fn set_taskbar_progress(
    window: WebviewWindow,
    status: String,
    progress: Option<u64>,
) -> Result<(), CommandError> {
    let status = match status.as_str() {
        "normal" => ProgressBarStatus::Normal,
        "error" => ProgressBarStatus::Error,
        "indeterminate" => ProgressBarStatus::Indeterminate,
        "paused" => ProgressBarStatus::Paused,
        _ => ProgressBarStatus::None,
    };

    window
        .set_progress_bar(ProgressBarState {
            status: Some(status),
            progress,
        })
        .map_err(|error| CommandError::io(error.to_string()))
}

/// Applies the light/dark native window theme. `"System"` follows the OS.
#[tauri::command]
pub fn set_window_theme(window: WebviewWindow, theme: String) -> Result<(), CommandError> {
    let theme = match theme.as_str() {
        "Light" => Some(tauri::Theme::Light),
        "Dark" => Some(tauri::Theme::Dark),
        _ => None,
    };

    window
        .set_theme(theme)
        .map_err(|error| CommandError::io(error.to_string()))
}

fn file_path_to_string(path: tauri_plugin_dialog::FilePath) -> Option<String> {
    path.as_path()
        .map(|path| path.to_string_lossy().into_owned())
}

/// Shallow JSON merge used by [`update_settings`].
fn merge(base: &mut Value, patch: Value) {
    if let (Some(base), Value::Object(patch)) = (base.as_object_mut(), patch) {
        for (key, value) in patch {
            base.insert(key, value);
        }
    }
}
