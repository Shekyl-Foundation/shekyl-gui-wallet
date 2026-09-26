// Copyright (c) 2026, The Shekyl Foundation
//
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification, are
// permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of
//    conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list
//    of conditions and the following disclaimer in the documentation and/or other
//    materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be
//    used to endorse or promote products derived from this software without specific
//    prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
// EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL
// THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
// PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
// THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! One clipboard placement, owned here.
//!
//! [`copy_to_clipboard`] writes text, records only a digest, and arms
//! [`ClipboardOwner::PLACEMENT_TTL`]. [`clear_clipboard`] and window teardown
//! clear early. The digest is forgotten when the clipboard no longer matches
//! it, or when the OS clear succeeds. A failed read or clear leaves the digest
//! tracked. The expiry task retries; window teardown is the last attempt.
//!
//! This is the recovery-phrase slot. It tracks one placement. Receive-address
//! copy stays in the webview and must not call these commands.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;
use zeroize::{Zeroize, Zeroizing};

/// Shown when the OS clipboard rejects a write, or the worker task panics.
const PLACE_FAILED: &str =
    "The recovery phrase could not be copied. Write it down from the screen.";

/// Identifies one successful placement. A newer placement supersedes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Generation(u64);

/// SHA-256 of text this process placed. The text itself is not retained.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ClipboardDigest([u8; 32]);

impl ClipboardDigest {
    pub(crate) fn of(text: &str) -> Self {
        Self(Sha256::digest(text.as_bytes()).into())
    }
}

impl Drop for ClipboardDigest {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

struct Placement {
    digest: ClipboardDigest,
    generation: Generation,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ClearScope {
    /// The placement currently tracked, whatever its generation.
    Current,
    /// Only if this generation is still the one tracked.
    Generation(Generation),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ClearOutcome {
    /// Nothing was tracked. The OS clipboard was not read or cleared.
    Idle,
    /// `scope` names a placement that has already been replaced.
    Superseded,
    /// The clipboard still held our text and was emptied.
    Cleared,
    /// The clipboard holds something else. Tracking stopped. It was not cleared.
    Released,
    /// The clipboard could not be read, or the clear failed. Tracking remains.
    StillTracked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ExpiryStep {
    Retry,
    Stop,
}

/// What the expiry task does after one attempt.
///
/// `attempts_completed` includes the attempt that just finished.
/// [`ClipboardOwner::CLEAR_ATTEMPT_LIMIT`] is that count, not a gap count.
pub(crate) fn expiry_step(outcome: ClearOutcome, attempts_completed: u32) -> ExpiryStep {
    if outcome == ClearOutcome::StillTracked
        && attempts_completed < ClipboardOwner::CLEAR_ATTEMPT_LIMIT
    {
        ExpiryStep::Retry
    } else {
        ExpiryStep::Stop
    }
}

#[derive(Default)]
pub(crate) struct PlacementLedger {
    current: Option<Placement>,
    next_generation: u64,
    /// Set when the window is destroyed. A later place must not write.
    closed: bool,
}

impl PlacementLedger {
    pub(crate) fn seal(&mut self) {
        self.closed = true;
    }

    pub(crate) fn record(&mut self, digest: ClipboardDigest) -> Generation {
        self.next_generation = self.next_generation.wrapping_add(1);
        let generation = Generation(self.next_generation);
        self.current = Some(Placement { digest, generation });
        generation
    }

    fn generation_for(&self, scope: ClearScope) -> Result<Generation, ClearOutcome> {
        let Some(placement) = &self.current else {
            return Err(ClearOutcome::Idle);
        };
        match scope {
            ClearScope::Current => Ok(placement.generation),
            ClearScope::Generation(generation) if generation == placement.generation => {
                Ok(generation)
            }
            ClearScope::Generation(_) => Err(ClearOutcome::Superseded),
        }
    }

    fn matches(&self, generation: Generation, digest: &ClipboardDigest) -> bool {
        self.current.as_ref().is_some_and(|placement| {
            placement.generation == generation && placement.digest == *digest
        })
    }

    fn forget(&mut self, generation: Generation) {
        if self
            .current
            .as_ref()
            .is_some_and(|placement| placement.generation == generation)
        {
            self.current = None;
        }
    }
}

pub(crate) trait ClipboardIo {
    fn write_text(&mut self, text: &str) -> Result<(), String>;
    fn read_text(&mut self) -> Result<Zeroizing<String>, ()>;
    fn clear(&mut self) -> Result<(), ()>;
}

pub(crate) fn apply_place(
    ledger: &mut PlacementLedger,
    io: &mut impl ClipboardIo,
    text: &str,
) -> Result<Generation, String> {
    if ledger.closed {
        return Err(PLACE_FAILED.to_string());
    }
    let digest = ClipboardDigest::of(text);
    io.write_text(text)?;
    Ok(ledger.record(digest))
}

/// Refuse later placements, then clear whatever is still tracked.
///
/// One lock covers both. A place that has not yet entered `apply_place`
/// observes `closed` and does not write; a place already inside it finishes
/// the write, and this clear then removes that text.
pub(crate) fn seal_and_clear(
    ledger: &mut PlacementLedger,
    io: &mut impl ClipboardIo,
) -> ClearOutcome {
    ledger.seal();
    apply_clear(ledger, io, ClearScope::Current)
}

/// Read, compare, and maybe clear while the caller holds the ledger lock.
///
/// The lock makes our own next placement wait until this attempt finishes,
/// so a clear cannot empty text a newer `apply_place` has already written.
pub(crate) fn apply_clear(
    ledger: &mut PlacementLedger,
    io: &mut impl ClipboardIo,
    scope: ClearScope,
) -> ClearOutcome {
    let generation = match ledger.generation_for(scope) {
        Ok(generation) => generation,
        Err(outcome) => return outcome,
    };
    let probe = match io.read_text() {
        Ok(text) => ClipboardDigest::of(text.as_str()),
        Err(()) => return ClearOutcome::StillTracked,
    };
    if ledger.matches(generation, &probe) {
        match io.clear() {
            Ok(()) => {
                ledger.forget(generation);
                ClearOutcome::Cleared
            }
            Err(()) => ClearOutcome::StillTracked,
        }
    } else {
        ledger.forget(generation);
        ClearOutcome::Released
    }
}

struct PluginClipboard {
    app: tauri::AppHandle,
}

impl ClipboardIo for PluginClipboard {
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.app
            .clipboard()
            .write_text(text)
            .map_err(|_| PLACE_FAILED.to_string())
    }

    fn read_text(&mut self) -> Result<Zeroizing<String>, ()> {
        self.app
            .clipboard()
            .read_text()
            .map(Zeroizing::new)
            .map_err(|_| ())
    }

    fn clear(&mut self) -> Result<(), ()> {
        self.app.clipboard().clear().map_err(|_| ())
    }
}

/// What [`copy_to_clipboard`] tells the page. The notice uses this delay.
/// The page does not schedule the clear.
#[derive(Debug, Serialize)]
pub struct ClipboardPlacement {
    pub clear_after_ms: u64,
}

/// Process-wide placement slot. Managed state, not wallet-session state.
#[derive(Clone)]
pub struct ClipboardOwner {
    ledger: Arc<Mutex<PlacementLedger>>,
    expiry: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
}

impl ClipboardOwner {
    /// How long a placement may sit before the first clear attempt.
    pub const PLACEMENT_TTL_MS: u64 = 60_000;
    pub const PLACEMENT_TTL: Duration = Duration::from_millis(Self::PLACEMENT_TTL_MS);
    /// Pause between attempts after a read or clear failure.
    pub const CLEAR_RETRY_GAP: Duration = Duration::from_secs(2);
    /// Attempts the expiry task makes. Window teardown tries once more.
    pub const CLEAR_ATTEMPT_LIMIT: u32 = 5;
    /// Longer than a recovery phrase. Bytes, so the check cannot split a scalar.
    pub const MAX_PLACED_TEXT_BYTES: usize = 1024;

    pub fn new() -> Self {
        Self {
            ledger: Arc::new(Mutex::new(PlacementLedger::default())),
            expiry: Arc::new(Mutex::new(None)),
        }
    }

    fn lock_ledger(&self) -> MutexGuard<'_, PlacementLedger> {
        // A panic while clearing must not poison the slot shut. The placement
        // is still the thing a later attempt has to clear.
        self.ledger
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn lock_expiry(&self) -> MutexGuard<'_, Option<tauri::async_runtime::JoinHandle<()>>> {
        self.expiry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn clear_scope_blocking(
        &self,
        app: &tauri::AppHandle,
        scope: ClearScope,
    ) -> ClearOutcome {
        let mut io = PluginClipboard { app: app.clone() };
        let mut ledger = self.lock_ledger();
        apply_clear(&mut ledger, &mut io, scope)
    }

    fn seal_and_clear_blocking(&self, app: &tauri::AppHandle) -> ClearOutcome {
        let mut io = PluginClipboard { app: app.clone() };
        let mut ledger = self.lock_ledger();
        seal_and_clear(&mut ledger, &mut io)
    }

    async fn clear_scope(&self, app: &tauri::AppHandle, scope: ClearScope) -> ClearOutcome {
        let owner = self.clone();
        let app = app.clone();
        match tauri::async_runtime::spawn_blocking(move || owner.clear_scope_blocking(&app, scope))
            .await
        {
            Ok(outcome) => outcome,
            // The worker panicked. Leave whatever the ledger still holds tracked.
            Err(_) => ClearOutcome::StillTracked,
        }
    }

    fn abort_expiry(&self) {
        if let Some(handle) = self.lock_expiry().take() {
            handle.abort();
        }
    }

    fn arm_expiry(&self, app: &tauri::AppHandle, generation: Generation) {
        // Abort outside the ledger lock. The task takes that lock to clear.
        self.abort_expiry();
        let owner = self.clone();
        let app = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Self::PLACEMENT_TTL).await;
            let mut attempts_completed = 0u32;
            loop {
                let outcome = owner
                    .clear_scope(&app, ClearScope::Generation(generation))
                    .await;
                attempts_completed = attempts_completed.saturating_add(1);
                if expiry_step(outcome, attempts_completed) == ExpiryStep::Stop {
                    break;
                }
                tokio::time::sleep(Self::CLEAR_RETRY_GAP).await;
            }
        });
        *self.lock_expiry() = Some(handle);
    }

    async fn place(&self, app: &tauri::AppHandle, text: Zeroizing<String>) -> Result<u64, String> {
        if text.is_empty() {
            return Err(
                "There is nothing to copy. Write the recovery phrase down from the screen.".into(),
            );
        }
        if text.len() > Self::MAX_PLACED_TEXT_BYTES {
            return Err(
                "That text is too long to copy. Write the recovery phrase down from the screen."
                    .into(),
            );
        }
        let owner = self.clone();
        let app_for_io = app.clone();
        let generation = tauri::async_runtime::spawn_blocking(move || {
            let mut io = PluginClipboard { app: app_for_io };
            let mut ledger = owner.lock_ledger();
            apply_place(&mut ledger, &mut io, text.as_str())
        })
        .await
        .map_err(|_| PLACE_FAILED.to_string())??;
        self.arm_expiry(app, generation);
        Ok(Self::PLACEMENT_TTL_MS)
    }
}

impl Default for ClipboardOwner {
    fn default() -> Self {
        Self::new()
    }
}

/// Write `text`, remember only its digest, and arm the clear.
///
/// Returns the delay the page should display. The clear does not depend on
/// that display timer.
#[tauri::command]
pub async fn copy_to_clipboard(
    app: tauri::AppHandle,
    owner: tauri::State<'_, ClipboardOwner>,
    text: String,
) -> Result<ClipboardPlacement, String> {
    let clear_after_ms = owner.inner().place(&app, Zeroizing::new(text)).await?;
    Ok(ClipboardPlacement { clear_after_ms })
}

/// Clear the current placement early (the create page is going away).
///
/// Returns whether the OS clipboard was emptied. `false` includes "nothing
/// tracked" and "the clipboard has changed"; neither reads as a failure.
/// A failed read or clear stays tracked and the expiry task keeps retrying.
#[tauri::command]
pub async fn clear_clipboard(
    app: tauri::AppHandle,
    owner: tauri::State<'_, ClipboardOwner>,
) -> Result<bool, String> {
    let owner = owner.inner().clone();
    let outcome = owner.clear_scope(&app, ClearScope::Current).await;
    if outcome != ClearOutcome::StillTracked {
        owner.abort_expiry();
    }
    Ok(outcome == ClearOutcome::Cleared)
}

/// Best-effort clear when the window is gone and the page timer is gone with it.
///
/// Runs off the main thread: `read_text` can deadlock there when reading back
/// text this process wrote. Does not `block_on` the Tauri runtime.
pub fn clear_on_window_destroy(app: &tauri::AppHandle) {
    let owner = app.state::<ClipboardOwner>().inner().clone();
    let app = app.clone();
    let _ = std::thread::spawn(move || {
        let outcome = owner.seal_and_clear_blocking(&app);
        // A failed read or clear is still tracked. Leave the expiry task so
        // it can retry if the process stays up. Every other outcome is final.
        if outcome != ClearOutcome::StillTracked {
            owner.abort_expiry();
        }
    })
    .join();
}

#[cfg(test)]
mod placement_tests {
    use super::{
        apply_clear, apply_place, expiry_step, seal_and_clear, ClearOutcome, ClearScope,
        ClipboardIo, ClipboardOwner, ExpiryStep, PlacementLedger,
    };
    use zeroize::Zeroizing;

    struct MemoryClipboard {
        contents: Option<String>,
        fail_read: bool,
        fail_clear: bool,
        fail_write: bool,
        reads: u32,
        clears: u32,
    }

    impl MemoryClipboard {
        fn new() -> Self {
            Self {
                contents: None,
                fail_read: false,
                fail_clear: false,
                fail_write: false,
                reads: 0,
                clears: 0,
            }
        }
    }

    impl ClipboardIo for MemoryClipboard {
        fn write_text(&mut self, text: &str) -> Result<(), String> {
            if self.fail_write {
                return Err("write failed".into());
            }
            self.contents = Some(text.to_owned());
            Ok(())
        }

        fn read_text(&mut self) -> Result<Zeroizing<String>, ()> {
            self.reads += 1;
            if self.fail_read {
                return Err(());
            }
            Ok(Zeroizing::new(self.contents.clone().unwrap_or_default()))
        }

        fn clear(&mut self) -> Result<(), ()> {
            self.clears += 1;
            if self.fail_clear {
                return Err(());
            }
            self.contents = None;
            Ok(())
        }
    }

    fn placed(text: &str) -> (PlacementLedger, MemoryClipboard) {
        let mut ledger = PlacementLedger::default();
        let mut io = MemoryClipboard::new();
        apply_place(&mut ledger, &mut io, text).expect("place");
        (ledger, io)
    }

    #[test]
    fn idle_clear_does_not_touch_the_clipboard() {
        let mut ledger = PlacementLedger::default();
        let mut io = MemoryClipboard::new();
        io.contents = Some("someone else's note".into());
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Idle
        );
        assert_eq!(io.reads, 0);
        assert_eq!(io.clears, 0);
        assert_eq!(io.contents.as_deref(), Some("someone else's note"));
    }

    #[test]
    fn failed_write_records_nothing() {
        let mut ledger = PlacementLedger::default();
        let mut io = MemoryClipboard::new();
        io.fail_write = true;
        assert!(apply_place(&mut ledger, &mut io, "alpha").is_err());
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Idle
        );
        assert_eq!(io.reads, 0);
        assert_eq!(io.clears, 0);
    }

    #[test]
    fn matching_text_is_cleared_and_forgotten() {
        let (mut ledger, mut io) = placed("alpha phrase");
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Cleared
        );
        assert_eq!(io.clears, 1);
        assert_eq!(io.contents, None);
        let reads_after = io.reads;
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Idle
        );
        assert_eq!(io.reads, reads_after);
    }

    #[test]
    fn different_text_is_left_in_place_and_forgotten() {
        let (mut ledger, mut io) = placed("alpha phrase");
        io.contents = Some("a later copy from another app".into());
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Released
        );
        assert_eq!(io.clears, 0);
        assert_eq!(
            io.contents.as_deref(),
            Some("a later copy from another app")
        );
        let reads_after = io.reads;
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Idle
        );
        assert_eq!(io.reads, reads_after);
    }

    #[test]
    fn unreadable_clipboard_keeps_the_placement() {
        let (mut ledger, mut io) = placed("alpha phrase");
        io.fail_read = true;
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::StillTracked
        );
        assert_eq!(io.clears, 0);
        io.fail_read = false;
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Cleared
        );
        assert_eq!(io.contents, None);
    }

    #[test]
    fn failed_clear_keeps_the_placement() {
        let (mut ledger, mut io) = placed("alpha phrase");
        io.fail_clear = true;
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::StillTracked
        );
        assert_eq!(io.contents.as_deref(), Some("alpha phrase"));
        io.fail_clear = false;
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Current),
            ClearOutcome::Cleared
        );
    }

    #[test]
    fn superseded_generation_does_not_clear_a_newer_placement() {
        let mut ledger = PlacementLedger::default();
        let mut io = MemoryClipboard::new();
        let first = apply_place(&mut ledger, &mut io, "alpha phrase").expect("first");
        let second = apply_place(&mut ledger, &mut io, "beta phrase").expect("second");
        assert_ne!(first, second);
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Generation(first)),
            ClearOutcome::Superseded
        );
        assert_eq!(io.clears, 0);
        assert_eq!(io.reads, 0);
        assert_eq!(io.contents.as_deref(), Some("beta phrase"));
        assert_eq!(
            apply_clear(&mut ledger, &mut io, ClearScope::Generation(second)),
            ClearOutcome::Cleared
        );
        assert_eq!(io.contents, None);
    }

    #[test]
    fn sealing_clears_a_placement_and_refuses_the_next_write() {
        let (mut ledger, mut io) = placed("alpha phrase");
        assert_eq!(seal_and_clear(&mut ledger, &mut io), ClearOutcome::Cleared);
        assert_eq!(io.contents, None);
        assert!(apply_place(&mut ledger, &mut io, "beta phrase").is_err());
        assert_eq!(io.contents, None);
        assert_eq!(io.clears, 1);
    }

    #[test]
    fn expiry_retries_only_while_the_placement_is_still_tracked() {
        let limit = ClipboardOwner::CLEAR_ATTEMPT_LIMIT;
        assert_eq!(
            expiry_step(ClearOutcome::StillTracked, limit - 1),
            ExpiryStep::Retry
        );
        assert_eq!(
            expiry_step(ClearOutcome::StillTracked, limit),
            ExpiryStep::Stop
        );
        for outcome in [
            ClearOutcome::Cleared,
            ClearOutcome::Released,
            ClearOutcome::Idle,
            ClearOutcome::Superseded,
        ] {
            assert_eq!(expiry_step(outcome, 1), ExpiryStep::Stop);
        }
    }
}
