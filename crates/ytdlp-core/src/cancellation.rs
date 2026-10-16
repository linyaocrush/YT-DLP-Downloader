//! Minimal cloneable async cancellation primitive.
//!
//! Mirrors the role of `System.Threading.CancellationToken`: it can be cloned,
//! triggered from any thread, and awaited by the process runner so that a
//! cancelled download kills its child process tree.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::watch;

/// A cloneable, one-shot cancellation signal.
#[derive(Clone, Debug)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
    sender: Arc<watch::Sender<bool>>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        // The receiver is dropped immediately; `send_replace` still updates the
        // stored value so a receiver created later via `subscribe` sees it.
        let (sender, _receiver) = watch::channel(false);
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(sender),
        }
    }
}

impl CancellationToken {
    /// Creates a token that has not been cancelled yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Safe to call repeatedly and from any thread.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.sender.send_replace(true);
    }

    /// Returns `true` once cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Resolves once cancellation is requested (immediately when already cancelled).
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }

        let mut receiver = self.sender.subscribe();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                break;
            }
        }
    }
}
