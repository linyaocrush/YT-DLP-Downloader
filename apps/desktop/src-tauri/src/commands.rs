//! Tauri commands. Each one is a thin shell over the ported core services; the
//! Svelte frontend owns the UI state and orchestration.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use serde_json::Value;
use tauri::window::{ProgressBarState, ProgressBarStatus};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;

use ytdlp_downloader_core::{
    locate_in_path, AppSettings, CancellationToken, DownloadKind, DownloadRequest, DownloadUpdate,
    ProgressCallback, ProxyListMode, ProxyRules, YtDlpFormat,
};

use crate::dto::{
    CookieFileDto, CookieListingDto, DenoStatusDto, DownloadOutcomeDto, DownloadRequestDto,
    DownloadUpdateDto, FfmpegStatusDto, MediaInfoDto, ParseResultDto, PlaylistInfoDto,
    ProxyIndicatorDto, YtDlpDetectionDto, YtDlpStatusDto,
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
    state
        .settings
        .save()
        .map_err(|error| CommandError::io(format!("设置保存失败：{error}")))?;
    Ok(updated)
}

#[tauri::command]
pub fn settings_warning(state: State<'_, AppState>) -> Option<String> {
    state.settings.load_warning()
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

    let timeout_token = CancellationToken::new();
    let version = match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        cli.get_version(timeout_token.clone()),
    )
    .await
    {
        Ok(result) => result.ok().flatten(),
        Err(_) => {
            timeout_token.cancel();
            None
        }
    };
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
pub async fn deno_status() -> DenoStatusDto {
    let Some(located) = locate_in_path("deno.exe") else {
        return DenoStatusDto {
            located_in_path: None,
            version: None,
            ready: false,
            status_text: "未在系统 PATH 中检测到 Deno。Deno 不是必需项，但 YouTube 的部分格式需要 JavaScript 运行时；安装后请重启本程序，让新的 PATH 生效。"
                .to_string(),
        };
    };

    let version = deno_version(&located).await;
    let ready = version.is_some();
    let status_text = match &version {
        Some(version) => format!("已在系统 PATH 中检测到 Deno：{located}（{version}）"),
        None => format!("已找到 Deno，但无法运行它，请确认安装完整：{located}"),
    };

    DenoStatusDto {
        located_in_path: Some(located),
        version,
        ready,
        status_text,
    }
}

/// Runs `deno --version` without flashing a console window.
async fn deno_version(path: &str) -> Option<String> {
    let mut command = tokio::process::Command::new(path);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.kill_on_drop(true);

    #[cfg(windows)]
    command.creation_flags(0x0800_0000);

    let output = tokio::time::timeout(std::time::Duration::from_secs(8), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let first_line = String::from_utf8_lossy(&output.stdout);
    let first_line = first_line.lines().next().unwrap_or_default().trim();
    (!first_line.is_empty()).then(|| first_line.to_string())
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
    let mut scan_error = None;
    let mut invalid = 0;
    if exists {
        match std::fs::read_dir(&folder) {
            Ok(entries) => {
                files = entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.path().is_file())
                    .filter(|entry| {
                        let valid = util::validate_cookie(&entry.path().to_string_lossy()).is_ok();
                        if !valid {
                            invalid += 1;
                        }
                        valid
                    })
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
            Err(error) => scan_error = Some(format!("无法读取 Cookie 文件夹：{error}")),
        }
    }

    let hint = if folder.is_empty() {
        "未设置 Cookie 文件夹。请到“设置”页选择一个文件夹，即可在此选择其中的 Cookie 文件。"
            .to_string()
    } else if let Some(error) = scan_error {
        error
    } else if exists && files.is_empty() {
        format!("未找到有效 Netscape Cookie 文件（忽略 {invalid} 个无效文件）。请将导出的 Cookie 放入该目录。")
    } else if exists {
        format!(
            "找到 {} 个有效 Cookie 文件，忽略 {invalid} 个无效文件。",
            files.len()
        )
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
    list_cookie_files(state).hint
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
pub async fn parse_media(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
) -> Result<ParseResultDto, CommandError> {
    if let Some(path) = &cookie_file {
        util::validate_cookie(path).map_err(CommandError::message)?;
    }
    let state = app.state::<AppState>();
    let job = state.begin_job()?;
    let (cli, settings_service, cancellation) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.cli),
            Arc::clone(&state.settings),
            job.token.clone(),
        )
    };

    let settings = settings_service.settings();
    let cookie = cookie_file.filter(|value| !value.trim().is_empty());
    let proxy = util::proxy_argument(&settings, &url);

    let result = cli
        .get_media_or_playlist(&url, cookie.as_deref(), proxy.as_deref(), cancellation)
        .await;

    let parsed = result?;
    Ok(ParseResultDto {
        media: parsed.media.as_ref().map(MediaInfoDto::from),
        playlist: parsed.playlist.as_ref().map(PlaylistInfoDto::from),
    })
}

/// Parses one concrete video (used for each queue item). Unlike [`parse_media`]
/// this never expands a playlist.
#[tauri::command]
pub async fn get_media_info(
    app: AppHandle,
    url: String,
    cookie_file: Option<String>,
) -> Result<MediaInfoDto, CommandError> {
    if let Some(path) = &cookie_file {
        util::validate_cookie(path).map_err(CommandError::message)?;
    }
    let state = app.state::<AppState>();
    let job = state.begin_job()?;
    let (cli, settings_service, cancellation) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.cli),
            Arc::clone(&state.settings),
            job.token.clone(),
        )
    };

    let settings = settings_service.settings();
    let cookie = cookie_file.filter(|value| !value.trim().is_empty());
    let proxy = util::proxy_argument(&settings, &url);

    let result = cli
        .get_media_info(&url, cookie.as_deref(), proxy.as_deref(), cancellation)
        .await;

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
    let state = app.state::<AppState>();
    let job = state.begin_job()?;
    run_download(&app, request, job.id, job.token.clone(), None).await
}

pub(crate) async fn run_download(
    app: &AppHandle,
    request: DownloadRequestDto,
    job_id: u64,
    cancellation: CancellationToken,
    item_id: Option<u64>,
) -> Result<DownloadOutcomeDto, CommandError> {
    // Build the format expression before claiming the job slot so a validation
    // error cannot leave a stale token behind.
    let expression = build_expression(&request)?;
    validate_request(&request)?;
    let state = app.state::<AppState>();

    let (cli, settings_service, cancellation) = {
        let state = app.state::<AppState>();
        (
            Arc::clone(&state.cli),
            Arc::clone(&state.settings),
            cancellation.clone(),
        )
    };

    let settings = settings_service.settings();
    let directory = if request.output_directory.trim().is_empty() {
        util::default_download_directory()
    } else {
        request.output_directory.trim().to_string()
    };

    let mut download = DownloadRequest::new(request.url.clone(), expression, directory);
    std::fs::create_dir_all(&download.output_directory)
        .map_err(|e| CommandError::io(format!("无法创建输出目录：{e}")))?;
    let probe = Path::new(&download.output_directory).join(format!(".ytdlp-write-check-{job_id}"));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|e| CommandError::io(format!("输出目录不可写：{e}")))?;
    drop(file);
    std::fs::remove_file(probe)
        .map_err(|e| CommandError::io(format!("无法清理目录检查文件：{e}")))?;
    download.cookie_file = request.cookie_file.clone();
    download.proxy_url = util::proxy_argument(&settings, &request.url);
    download.concurrent_fragments = settings.download_threads.clamp(1, 64);
    download.output_template = request.output_template.clone();
    download.write_thumbnail = request.write_thumbnail;
    download.live = request.live;
    let info = cli
        .get_media_info(
            &request.url,
            request.cookie_file.as_deref(),
            download.proxy_url.as_deref(),
            cancellation.clone(),
        )
        .await?;
    if info.is_upcoming {
        return Err(CommandError::message("预约直播尚未开始，请稍后重试"));
    }
    download.live = info.is_live;
    if info.videos.is_empty() && info.audios.is_empty() {
        return Err(CommandError::message("当前没有可下载格式，请稍后重新解析"));
    }
    let needs_merge = expression_requires_ffmpeg(&download.format_expression);
    if needs_merge {
        let path = state
            .ffmpeg_paths
            .locate_in_path()
            .or_else(|| state.ffmpeg_paths.configured_path())
            .ok_or_else(|| CommandError::message("当前下载需要 ffmpeg 合并，请先在设置中配置"))?;
        let mut command = tokio::process::Command::new(path);
        command
            .arg("-version")
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        if !tokio::time::timeout(std::time::Duration::from_secs(8), command.status())
            .await
            .ok()
            .and_then(Result::ok)
            .is_some_and(|status| status.success())
        {
            return Err(CommandError::message(
                "ffmpeg 无法运行或检测超时，请检查安装",
            ));
        }
    }

    if !request.auto {
        let selection = request
            .manual
            .as_ref()
            .ok_or_else(|| CommandError::message("缺少手动选流"))?;
        if selection
            .video_format_id
            .as_ref()
            .is_some_and(|id| !info.videos.iter().any(|v| &v.format_id == id))
            || selection
                .audio_format_id
                .as_ref()
                .is_some_and(|id| !info.audios.iter().any(|a| &a.format_id == id))
        {
            return Err(CommandError::message("格式已变化，请重新解析并选择"));
        }
    }
    let emitter = app.clone();
    let progress: ProgressCallback = Arc::new(move |update: DownloadUpdate| {
        let mut dto = DownloadUpdateDto::from(update);
        dto.job_id = job_id;
        dto.item_id = item_id;
        let _ = emitter.emit(DOWNLOAD_UPDATE_EVENT, dto);
    });

    let result = cli.download(download, Some(progress), cancellation).await;

    let outcome = result?;
    Ok(DownloadOutcomeDto {
        success: outcome.success,
        cancelled: outcome.cancelled,
        output_path: outcome.output_path,
        error: outcome.error,
    })
}

#[tauri::command]
pub fn cancel_job(app: AppHandle, job_id: Option<u64>) {
    app.state::<AppState>().cancel_job(job_id);
}

#[tauri::command]
pub fn job_status(state: State<'_, AppState>) -> Option<crate::state::JobStatus> {
    state.job_status()
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
    if !Path::new(&path).exists() {
        return Err(CommandError::message("文件不存在，可能已移动或删除"));
    }
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|error| CommandError::io(error.to_string()))
}

pub(crate) fn validate_request(request: &DownloadRequestDto) -> Result<(), CommandError> {
    url_check(&request.url)?;
    if let Some(path) = &request.cookie_file {
        util::validate_cookie(path).map_err(CommandError::message)?;
    }
    if let Some(template) = &request.output_template {
        validate_output_template(template)?;
    }
    Ok(())
}

fn validate_output_template(template: &str) -> Result<(), CommandError> {
    let base_name = template.split('.').next().unwrap_or_default();
    let reserved = ["CON", "PRN", "AUX", "NUL"]
        .into_iter()
        .any(|name| base_name.eq_ignore_ascii_case(name))
        || (1..=9).any(|index| {
            base_name.eq_ignore_ascii_case(&format!("COM{index}"))
                || base_name.eq_ignore_ascii_case(&format!("LPT{index}"))
        });
    if template.trim().is_empty()
        || template.len() > 240
        || template == "."
        || template == ".."
        || template.ends_with([' ', '.'])
        || template.chars().any(|character| {
            character.is_control()
                || ['/', '\\', ':', '"', '<', '>', '|', '?', '*'].contains(&character)
        })
        || reserved
    {
        return Err(CommandError::message(
            "文件名无效：请避免路径分隔符、Windows 保留名、控制字符及过长名称",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod request_validation_tests {
    use super::validate_output_template;

    #[test]
    fn validates_windows_file_names_without_rejecting_yt_dlp_tokens() {
        assert!(validate_output_template("%(title)s.%(ext)s").is_ok());
        assert!(validate_output_template("video - 001.%(ext)s").is_ok());
        for invalid in [
            "", "..", "../video", "name.", "name ", "CON.txt", "bad?.mp4",
        ] {
            assert!(validate_output_template(invalid).is_err(), "{invalid}");
        }
    }
}

fn expression_requires_ffmpeg(expression: &str) -> bool {
    expression.contains('+')
}

fn url_check(url: &str) -> Result<&str, CommandError> {
    if !url::Url::parse(url).ok().is_some_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()
    }) {
        return Err(CommandError::message("请输入有效的 HTTP / HTTPS 视频链接"));
    }
    Ok(url)
}

#[tauri::command]
pub fn open_directory(app: AppHandle, path: String) -> Result<(), CommandError> {
    if !Path::new(&path).is_dir() {
        return Err(CommandError::message("输出目录不存在"));
    }
    app.opener()
        .open_path(path, None::<String>)
        .map_err(|error| CommandError::io(error.to_string()))
}

#[tauri::command]
pub async fn prepare_exit(app: AppHandle) -> Result<(), CommandError> {
    app.state::<AppState>().cancel_job(None);
    for _ in 0..100 {
        if app.state::<AppState>().job_status().is_none() {
            app.exit(0);
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err(CommandError::message("任务仍在停止，请稍后再次关闭窗口"))
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
