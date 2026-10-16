//! Ported from `YtDlpDownloader.Core.Tests/Services/ProcessRunnerTests.cs`.
//!
//! These exercise real `cmd.exe`/`ping` processes, so they only run on Windows.

#![cfg(windows)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ytdlp_downloader_core::{CancellationToken, ProcessError, ProcessRunner, TokioProcessRunner};

mod common;
use common::TempDir;

fn system_executable(name: &str) -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    PathBuf::from(root).join("System32").join(name)
}

fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[tokio::test]
async fn run_missing_executable_errors() {
    let temp = TempDir::new();
    let runner = TokioProcessRunner;

    let result = runner
        .run(
            &temp.path().join("missing.exe").to_string_lossy(),
            &[],
            None,
            None,
            CancellationToken::new(),
        )
        .await;

    assert!(matches!(result, Err(ProcessError::Io(_))));
}

#[tokio::test]
async fn run_echo_returns_stdout_and_exit_code() {
    let runner = TokioProcessRunner;
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));

    let sink = Arc::clone(&lines);
    let callback = Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_string()));

    let result = runner
        .run(
            &system_executable("cmd.exe").to_string_lossy(),
            &arguments(&["/c", "echo", "hello-world"]),
            Some(callback),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert!(!result.cancelled);
    assert_eq!(result.exit_code, 0);
    assert!(result.standard_output.contains("hello-world"));
    assert!(lines
        .lock()
        .unwrap()
        .iter()
        .any(|line| line == "hello-world"));
}

#[tokio::test]
async fn run_non_zero_exit_captures_stderr() {
    let runner = TokioProcessRunner;

    let result = runner
        .run(
            &system_executable("cmd.exe").to_string_lossy(),
            &arguments(&["/c", "echo", "oops", "1>&2", "&", "exit", "3"]),
            None,
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(result.exit_code, 3);
    assert!(result.standard_error.contains("oops"));
}

#[tokio::test]
async fn run_cancelled_kills_process_and_reports_cancellation() {
    let runner = TokioProcessRunner;
    let token = CancellationToken::new();

    let canceller = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        canceller.cancel();
    });

    let result = runner
        .run(
            &system_executable("cmd.exe").to_string_lossy(),
            &arguments(&["/c", "ping", "-n", "8", "127.0.0.1"]),
            None,
            None,
            token,
        )
        .await
        .unwrap();

    assert!(result.cancelled);
}
