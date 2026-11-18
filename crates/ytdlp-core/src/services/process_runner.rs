//! Async process runner with UTF-8 line streaming and tree-kill cancellation.

use std::fmt;
use std::io;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

use crate::cancellation::CancellationToken;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Callback invoked once per captured output line (on the runner's async task).
pub type LineCallback = Arc<dyn Fn(&str) + Send + Sync + 'static>;

const CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;
const CAPTURE_TRUNCATION_MARKER: &str = "\n[输出过长，已省略较早内容]\n";
const CAPTURE_TAIL_BYTES: usize = CAPTURE_LIMIT_BYTES - CAPTURE_TRUNCATION_MARKER.len();
const PROCESS_CLEANUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Outcome of a finished (or cancelled) process run.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessResult {
    /// True when the cancellation token was already triggered.
    pub cancelled: bool,
    pub exit_code: i32,
    /// Captured stdout, one line per `\n`, with a trailing newline.
    pub standard_output: String,
    /// Captured stderr, one line per `\n`, with a trailing newline.
    pub standard_error: String,
}

/// Failure to even start or supervise a process.
#[derive(Debug)]
pub enum ProcessError {
    /// The run was cancelled before a normal exit was observed.
    Cancelled,
    /// The process could not be spawned or reaped (e.g. missing executable).
    Io(io::Error),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessError::Cancelled => write!(f, "进程已取消"),
            ProcessError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProcessError::Io(error) => Some(error),
            ProcessError::Cancelled => None,
        }
    }
}

impl From<io::Error> for ProcessError {
    fn from(error: io::Error) -> Self {
        ProcessError::Io(error)
    }
}

/// Starts an executable with UTF-8 redirected stdout/stderr, streaming each line
/// to the supplied callbacks. Cancellation kills the process tree.
#[async_trait]
pub trait ProcessRunner: Send + Sync {
    async fn run(
        &self,
        file_name: &str,
        arguments: &[String],
        on_stdout: Option<LineCallback>,
        on_stderr: Option<LineCallback>,
        cancellation: CancellationToken,
    ) -> Result<ProcessResult, ProcessError>;
}

/// Tokio-backed [`ProcessRunner`].
pub struct TokioProcessRunner;

#[async_trait]
impl ProcessRunner for TokioProcessRunner {
    async fn run(
        &self,
        file_name: &str,
        arguments: &[String],
        on_stdout: Option<LineCallback>,
        on_stderr: Option<LineCallback>,
        cancellation: CancellationToken,
    ) -> Result<ProcessResult, ProcessError> {
        let mut command = Command::new(file_name);
        if cancellation.is_cancelled() {
            return Err(ProcessError::Cancelled);
        }
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.kill_on_drop(true);

        // Keep console windows from flashing when the GUI launches yt-dlp.
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);

        let mut child = command.spawn().map_err(ProcessError::Io)?;
        let pid = child.id();

        let stdout_buffer = Arc::new(Mutex::new(String::new()));
        let stderr_buffer = Arc::new(Mutex::new(String::new()));

        let stdout_task = child
            .stdout
            .take()
            .map(|stream| tokio::spawn(pump(stream, on_stdout, Arc::clone(&stdout_buffer))));
        let stderr_task = child
            .stderr
            .take()
            .map(|stream| tokio::spawn(pump(stream, on_stderr, Arc::clone(&stderr_buffer))));

        // Only the wait branch touches `child`; the cancellation branch merely
        // sets a flag, so the mutable borrow of `child` is released afterwards.
        let mut cancelled = false;
        let mut exit_code: Option<i32> = None;
        tokio::select! {
            result = child.wait() => {
                exit_code = Some(result.map_err(ProcessError::Io)?.code().unwrap_or(-1));
            }
            _ = cancellation.cancelled() => {
                cancelled = true;
            }
        }

        if cancelled {
            kill_tree(pid);
            let _ = child.start_kill();
            let status = tokio::time::timeout(PROCESS_CLEANUP_TIMEOUT, child.wait())
                .await
                .map_err(|_| {
                    ProcessError::Io(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "等待子进程退出超时",
                    ))
                })?
                .map_err(ProcessError::Io)?;
            exit_code = Some(status.code().unwrap_or(-1));
        }

        for mut task in [stdout_task, stderr_task].into_iter().flatten() {
            if tokio::time::timeout(PROCESS_CLEANUP_TIMEOUT, &mut task)
                .await
                .is_err()
            {
                kill_tree(pid);
                task.abort();
                let _ = task.await;
            }
        }

        Ok(ProcessResult {
            cancelled: cancelled || cancellation.is_cancelled(),
            exit_code: exit_code.unwrap_or(-1),
            standard_output: take_string(&stdout_buffer),
            standard_error: take_string(&stderr_buffer),
        })
    }
}

async fn pump<R>(reader: R, callback: Option<LineCallback>, buffer: Arc<Mutex<String>>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(mut guard) = buffer.lock() {
            append_capture(&mut guard, &line);
        }
        if let Some(callback) = &callback {
            callback(&line);
        }
    }
}

fn append_capture(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push('\n');
    if buffer.len() > CAPTURE_LIMIT_BYTES {
        let mut start = buffer.len() - CAPTURE_TAIL_BYTES;
        while !buffer.is_char_boundary(start) {
            start += 1;
        }
        buffer.replace_range(..start, CAPTURE_TRUNCATION_MARKER);
    }
}

fn take_string(buffer: &Arc<Mutex<String>>) -> String {
    buffer.lock().map(|guard| guard.clone()).unwrap_or_default()
}

/// Kills an entire process tree so a cancelled yt-dlp does not leave ffmpeg behind.
fn kill_tree(pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(pid) = pid {
        // `Child::kill` only terminates the direct child; taskkill /T walks the tree.
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    #[cfg(not(windows))]
    let _ = pid;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_keeps_tail_with_bounded_size_and_marker() {
        let mut buffer = String::new();
        for _ in 0..20 {
            append_capture(&mut buffer, &"x".repeat(64 * 1024));
        }
        assert!(buffer.len() <= CAPTURE_LIMIT_BYTES);
        assert!(buffer.contains(CAPTURE_TRUNCATION_MARKER));
        assert!(buffer.ends_with(&format!("{}\n", "x".repeat(64 * 1024))));
    }
}
