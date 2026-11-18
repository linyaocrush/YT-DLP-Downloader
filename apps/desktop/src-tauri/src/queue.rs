//! Durable sequential queue. The backend owns scheduling, retries and cancellation.
use crate::{commands, dto::DownloadRequestDto, error::CommandError, state::AppState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};
use ytdlp_downloader_core::settings_service::atomic_write;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum QueueItemState {
    Pending,
    Parsing,
    Parsed,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueItemRecord {
    id: u64,
    url: String,
    state: QueueItemState,
}

fn path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("YtDlpDownloader")
        .join("queue.json")
}

fn save(items: &[Value]) -> Result<(), CommandError> {
    atomic_write(
        &path(),
        &serde_json::to_vec_pretty(items).map_err(|e| CommandError::io(e.to_string()))?,
    )
    .map_err(|e| CommandError::io(format!("队列保存失败：{e}")))
}

fn recover(bytes: &[u8]) -> Result<Vec<Value>, CommandError> {
    let mut items: Vec<Value> = serde_json::from_slice(bytes)
        .map_err(|e| CommandError::message(format!("队列文件损坏：{e}")))?;
    validate_items(&items)?;
    for item in &mut items {
        let record: QueueItemRecord = serde_json::from_value(item.clone())
            .map_err(|_| CommandError::message("队列记录格式无效"))?;
        if matches!(
            record.state,
            QueueItemState::Downloading | QueueItemState::Parsing | QueueItemState::Parsed
        ) {
            item["state"] = json!("Pending");
        }
        item["videos"] = json!([]);
        item["audios"] = json!([]);
        item["selectedVideo"] = Value::Null;
        item["selectedAudio"] = Value::Null;
    }
    Ok(items)
}

fn validate_items(items: &[Value]) -> Result<(), CommandError> {
    let mut ids = std::collections::HashSet::new();
    for item in items {
        let record: QueueItemRecord = serde_json::from_value(item.clone())
            .map_err(|_| CommandError::message("队列记录格式无效"))?;
        if !ids.insert(record.id) || record.url.trim().is_empty() {
            return Err(CommandError::message("队列记录无效或包含重复 ID"));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn load_queue(state: State<'_, AppState>) -> Result<Vec<Value>, CommandError> {
    let mut queue = state
        .queue
        .lock()
        .map_err(|_| CommandError::message("队列状态不可用"))?;
    if queue.is_empty() {
        match std::fs::read(path()) {
            Ok(bytes) => {
                *queue = recover(&bytes).map_err(|error| {
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    let backup = path().with_extension(format!("corrupt-{stamp}.json"));
                    let detail = match std::fs::write(&backup, &bytes) {
                        Ok(()) => format!("备份：{}", backup.display()),
                        Err(e) => format!("备份失败：{e}"),
                    };
                    CommandError::message(format!("{error}；{detail}。请检查原文件后重新启动。"))
                })?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(CommandError::io(e.to_string())),
        }
    }
    Ok(queue.clone())
}

#[tauri::command]
pub fn save_queue(state: State<'_, AppState>, mut items: Vec<Value>) -> Result<(), CommandError> {
    if state.job_status().is_some() {
        return Err(CommandError::message("任务运行期间不能替换队列"));
    }
    validate_items(&items)?;
    for item in &mut items {
        if item["savedRequest"].is_object() {
            item["savedRequest"]["cookieFile"] = Value::Null;
        }
    }
    save(&items)?;
    *state
        .queue
        .lock()
        .map_err(|_| CommandError::message("队列状态不可用"))? = items;
    Ok(())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueRequest {
    pub item_id: u64,
    pub request: DownloadRequestDto,
}

#[derive(Serialize)]
pub struct QueueOutcome {
    completed: usize,
    failed: usize,
    cancelled: bool,
}

fn publish(app: &AppHandle, job_id: u64, item_id: u64, patch: Value) -> Result<(), CommandError> {
    let state = app.state::<AppState>();
    let mut queue = state
        .queue
        .lock()
        .map_err(|_| CommandError::message("队列状态不可用"))?;
    if let Some(item) = queue
        .iter_mut()
        .find(|item| item["id"].as_u64() == Some(item_id))
    {
        if let (Some(target), Some(patch)) = (item.as_object_mut(), patch.as_object()) {
            target.extend(patch.clone());
        }
    }
    save(&queue)?;
    app.emit(
        "queue:update",
        json!({"jobId": job_id, "itemId": item_id, "patch": patch}),
    )
    .map_err(|e| CommandError::io(e.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    Network,
    Authentication,
    Format,
    FileSystem,
    Other,
}

fn classify_failure(message: &str) -> FailureKind {
    let lower = message.to_lowercase();
    if [
        "timed out",
        "timeout",
        "connection reset",
        "connection aborted",
        "temporary failure",
        "http error 429",
        "http error 500",
        "http error 502",
        "http error 503",
        "http error 504",
        "超时",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        FailureKind::Network
    } else if [
        "login required",
        "sign in",
        "authentication",
        "unauthorized",
        "forbidden",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        FailureKind::Authentication
    } else if [
        "requested format",
        "format is not available",
        "no suitable formats",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        FailureKind::Format
    } else if [
        "permission denied",
        "access is denied",
        "no space left",
        "cannot write",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        FailureKind::FileSystem
    } else {
        FailureKind::Other
    }
}

fn retryable(message: &str) -> bool {
    classify_failure(message) == FailureKind::Network
}

#[tauri::command]
pub async fn start_queue(
    app: AppHandle,
    requests: Vec<QueueRequest>,
) -> Result<QueueOutcome, CommandError> {
    let state = app.state::<AppState>();
    let job = state.begin_job()?;
    {
        let mut items = state
            .queue
            .lock()
            .map_err(|_| CommandError::message("队列状态不可用"))?;
        for entry in &requests {
            if !items
                .iter()
                .any(|item| item["id"].as_u64() == Some(entry.item_id))
            {
                return Err(CommandError::message("任务不在持久化队列中"));
            }
            commands::validate_request(&entry.request)?;
        }
        for entry in &requests {
            let item = items
                .iter_mut()
                .find(|item| item["id"].as_u64() == Some(entry.item_id))
                .unwrap();
            item["savedRequest"] = serde_json::to_value(&entry.request)
                .map_err(|e| CommandError::io(e.to_string()))?;
            item["savedRequest"]["cookieFile"] = Value::Null;
        }
        save(&items)?;
    }
    let mut outcome = QueueOutcome {
        completed: 0,
        failed: 0,
        cancelled: false,
    };
    for entry in requests {
        if job.token.is_cancelled() {
            outcome.cancelled = true;
            break;
        }
        let mut persisted_request =
            serde_json::to_value(&entry.request).map_err(|e| CommandError::io(e.to_string()))?;
        persisted_request["cookieFile"] = Value::Null;
        publish(
            &app,
            job.id,
            entry.item_id,
            json!({"state": "Downloading", "progress": 0, "error": null, "savedRequest": persisted_request}),
        )?;
        let mut attempt = 0;
        loop {
            let result = commands::run_download(
                &app,
                entry.request.clone(),
                job.id,
                job.token.clone(),
                Some(entry.item_id),
            )
            .await;
            let (success, cancelled, output_path, error) = match result {
                Ok(result) => (
                    result.success,
                    result.cancelled,
                    result.output_path,
                    result.error,
                ),
                Err(error) => (
                    false,
                    job.token.is_cancelled(),
                    None,
                    Some(error.to_string()),
                ),
            };
            if cancelled || job.token.is_cancelled() {
                outcome.cancelled = true;
                publish(&app, job.id, entry.item_id, json!({"state": "Cancelled"}))?;
                break;
            }
            if success {
                outcome.completed += 1;
                publish(
                    &app,
                    job.id,
                    entry.item_id,
                    json!({"state": "Completed", "progress": 100, "outputPath": output_path, "error": null}),
                )?;
                break;
            }
            let error = error.unwrap_or_else(|| "未知错误".into());
            if attempt < 2 && retryable(&error) {
                attempt += 1;
                publish(
                    &app,
                    job.id,
                    entry.item_id,
                    json!({"error": format!("网络错误，{} 秒后重试（{attempt}/2）：{error}", 1 << attempt)}),
                )?;
                tokio::select! {
                    _ = job.token.cancelled() => {},
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1 << attempt)) => {},
                }
                if job.token.is_cancelled() {
                    outcome.cancelled = true;
                    publish(&app, job.id, entry.item_id, json!({"state": "Cancelled"}))?;
                    break;
                }
                continue;
            }
            outcome.failed += 1;
            publish(
                &app,
                job.id,
                entry.item_id,
                json!({"state": "Failed", "error": error}),
            )?;
            break;
        }
        if outcome.cancelled {
            break;
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_only_transient_errors() {
        assert!(retryable("HTTP Error 503: Service Unavailable"));
        assert!(retryable("connection reset by peer"));
        assert_eq!(
            classify_failure("login required"),
            FailureKind::Authentication
        );
        assert_eq!(
            classify_failure("requested format unavailable"),
            FailureKind::Format
        );
        assert_eq!(
            classify_failure("permission denied"),
            FailureKind::FileSystem
        );
        assert_eq!(classify_failure("video deleted"), FailureKind::Other);
        assert!(!retryable("login required"));
    }

    #[tokio::test]
    async fn cancellation_interrupts_retry_wait() {
        let token = ytdlp_downloader_core::CancellationToken::new();
        token.cancel();
        let cancelled = tokio::time::timeout(std::time::Duration::from_millis(100), async {
            tokio::select! {
                _ = token.cancelled() => true,
                _ = tokio::time::sleep(std::time::Duration::from_secs(4)) => false,
            }
        })
        .await
        .unwrap();
        assert!(cancelled);
    }

    #[test]
    fn recovery_preserves_options_and_history_but_invalidates_formats() {
        let items = recover(br#"[{"id":1,"url":"https://example.com/1","state":"Downloading","videos":[{}],"selectedVideo":{},"savedRequest":{"auto":false,"outputTemplate":"fixed.%(ext)s"}},{"id":2,"url":"https://example.com/2","state":"Completed","outputPath":"C:/out.mp4"}]"#).unwrap();
        assert_eq!(items[0]["state"], "Pending");
        assert_eq!(items[0]["videos"], json!([]));
        assert!(items[0]["selectedVideo"].is_null());
        assert_eq!(items[0]["savedRequest"]["outputTemplate"], "fixed.%(ext)s");
        assert_eq!(items[1]["state"], "Completed");
        assert_eq!(items[1]["outputPath"], "C:/out.mp4");
        assert!(recover(br#"[null]"#).is_err());
        assert!(recover(br#"[{"id":1,"url":"https://example.com/1","state":"Unknown"}]"#).is_err());
        assert!(recover(br#"[{"id":1,"url":"","state":"Pending"}]"#).is_err());
        assert!(recover(br#"[{"id":1,"url":"https://example.com/1","state":"Pending"},{"id":1,"url":"https://example.com/2","state":"Pending"}]"#).is_err());
    }
}
