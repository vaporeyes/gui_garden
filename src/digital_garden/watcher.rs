// Native-only filesystem watcher for the notes directory.
//
// Produces a debounced "something changed" signal that the digital garden
// polls each frame to trigger a re-scan. Wasm has no filesystem to watch,
// so the type is a stub there that always reports "no changes".

// Wasm builds keep the struct shell for API compatibility with `mod.rs`
// but none of the debounce/notify plumbing runs.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

#[cfg(not(target_arch = "wasm32"))]
use notify::{RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Default minimum interval between re-scans; keeps a burst of "file saved"
/// events from turning into a storm of full directory rebuilds. Can be
/// overridden at runtime via `set_debounce_ms`.
pub const DEFAULT_DEBOUNCE_MS: u64 = 300;

pub struct DirectoryWatcher {
    #[cfg(not(target_arch = "wasm32"))]
    _watcher: RecommendedWatcher,
    #[cfg(not(target_arch = "wasm32"))]
    rx: Receiver<()>,
    last_reload: Option<Instant>,
    debounce: Duration,
}

impl DirectoryWatcher {
    /// Spin up a watcher on `path`. Returns `None` on wasm or if the
    /// platform watcher rejects the path (e.g. permission denied).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<P: AsRef<Path>>(path: P, debounce_ms: u64) -> Option<Self> {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(
            move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    // Only react to markdown changes — avoids echoing on
                    // irrelevant editor swap files, .DS_Store, etc.
                    let relevant = ev.paths.iter().any(|p| {
                        p.extension().and_then(|e| e.to_str()) == Some("md")
                    });
                    if relevant {
                        let _ = tx.send(());
                    }
                }
            },
        )
        .ok()?;
        watcher
            .watch(path.as_ref(), RecursiveMode::Recursive)
            .ok()?;

        Some(Self {
            _watcher: watcher,
            rx,
            last_reload: None,
            debounce: Duration::from_millis(debounce_ms),
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new<P: AsRef<Path>>(_path: P, _debounce_ms: u64) -> Option<Self> {
        None
    }

    /// Adjust the debounce window at runtime (e.g. from Settings).
    pub fn set_debounce_ms(&mut self, ms: u64) {
        self.debounce = Duration::from_millis(ms);
    }

    /// Drain any pending events and, if at least one fired AND the debounce
    /// window has elapsed since the last reload, return true. Otherwise the
    /// caller should do nothing this frame.
    pub fn consume_if_ready(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut any = false;
            loop {
                match self.rx.try_recv() {
                    Ok(()) => any = true,
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
                }
            }
            if !any {
                return false;
            }
            let now = Instant::now();
            if let Some(last) = self.last_reload {
                if now.duration_since(last) < self.debounce {
                    return false;
                }
            }
            self.last_reload = Some(now);
            true
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }
}
