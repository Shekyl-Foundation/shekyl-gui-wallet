// Copyright (c) 2025-2026, The Shekyl Foundation
//
// All rights reserved.
// BSD-3-Clause

//! Launching the wallet's own node, holding its owning connection
//! (`docs/design/GUI_SYNC_LANE.md` GS-2; shekyl-core `INPUT_GRAMMAR.md`
//! IG-Q106).
//!
//! The node inherits one end of a connected pair and this process keeps the
//! other. That connection is the node's life and its readiness, and nothing
//! else:
//!
//! - **Ready**: the node writes its ready line once its owner's leg is
//!   served. A start that refuses writes none and exits; the read then ends,
//!   and the node's standard error says why.
//! - **Stop**: shutting down this end's writing half asks the node to stop.
//!   This end then reads end of stream when the node's process is gone, its
//!   rendezvous removed and its data directory unlocked. That read, not a
//!   timer, is when the next node may start.
//! - **This process's exit or death** closes the connection, and the node
//!   stops cleanly on its own. Nothing here kills a node at exit.
//!
//! The one kill is [`OwnNode::force_stop`], which a person asks for.
//!
//! The launch goes through `shekyl-spawn`, core's one launcher: the child
//! keeps its end and its standard streams and nothing else, and on Windows
//! the handle is inheritable only for the length of this one spawn, under
//! the process's one spawn lock.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};

use shekyl_spawn::inherit::stdio_only;
use shekyl_spawn::owning::read_ready;
use shekyl_spawn::pair::{keep_in_child, End, EndPair, WindowError};
use tokio::sync::watch;

/// Spacing of the looks for the node's exit status once its connection has
/// ended. The exit is already under way then; this only reaps it.
const REAP_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// Where a launched node is in its life, as its owning connection says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Life {
    /// Started; the ready line has not come.
    Starting,
    /// The ready line came: the owner's leg is served.
    Ready,
    /// The connection ended: the node's process is gone. `refusal` is what
    /// it wrote on standard error, when it wrote anything.
    Gone { refusal: Option<String> },
}

/// What to launch.
pub(crate) struct Launch {
    /// The node's program.
    pub program: PathBuf,
    /// Its arguments, without `--owning-connection`.
    pub args: Vec<OsString>,
    /// The file its standard error is written to, replaced at each launch.
    pub stderr: PathBuf,
}

/// A node this process launched and holds by its owning connection.
pub(crate) struct OwnNode {
    ours: End,
    child: Arc<Mutex<Child>>,
    life: watch::Receiver<Life>,
    pid: u32,
}

impl OwnNode {
    /// The node's process id, for the log.
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// The node's life, as it changes.
    pub(crate) fn life(&self) -> watch::Receiver<Life> {
        self.life.clone()
    }

    /// Ask the node to stop: shut down this end's writing half. The node
    /// reads end of stream and stops cleanly; [`Life::Gone`] follows when
    /// its process is gone. A node already gone has nothing to be asked.
    pub(crate) fn ask_to_stop(&self) {
        if let Err(e) = self.ours.shutdown(std::net::Shutdown::Write) {
            // The connection is already down: the node is gone or going.
            tracing::debug!(pid = self.pid, "the node's owning connection: {e}");
        }
    }

    /// Kill the node. Only ever on a person's request: a killed node writes
    /// nothing back, so its next start is a cold one.
    pub(crate) fn force_stop(&self) -> Result<(), String> {
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => child
                .kill()
                .map_err(|e| format!("The node could not be stopped: {e}")),
            Err(e) => Err(format!("The node's state could not be read: {e}")),
        }
    }
}

/// Start the node, holding its owning connection.
///
/// Returns once the process is started. Its readiness and its end arrive on
/// [`OwnNode::life`], from a thread that reads the connection for as long as
/// the node lives.
pub(crate) fn launch(launch: &Launch) -> Result<OwnNode, String> {
    let stderr = std::fs::File::create(&launch.stderr).map_err(|e| {
        format!(
            "The node's error file {} could not be written: {e}",
            launch.stderr.display()
        )
    })?;
    let mut command = Command::new(&launch.program);
    command
        .args(&launch.args)
        .stdin(Stdio::null())
        // Not pipes: this process may exit while the node is still
        // stopping, and a node writing into a closed pipe would end on the
        // write instead of on its clean stop.
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr));
    no_console_window(&mut command);

    let pair = EndPair::new().map_err(|e| format!("The node could not be started: {e}"))?;
    let (child, ours) = match EndPair::spawn(pair, |number| {
        command.arg("--owning-connection").arg(number.to_string());
        // In this order: every descriptor above the standard streams is
        // marked close-on-exec, then the node's end alone is kept. The
        // wallet's process holds a webview's descriptors; the node gets
        // none of them.
        stdio_only(&mut command);
        keep_in_child(&mut command, number);
        command.spawn()
    }) {
        Ok(started) => started,
        Err(WindowError::Open(e)) => return Err(format!("The node could not be started: {e}")),
        Err(WindowError::Spawn { error, .. }) => {
            return Err(format!(
                "The node at {} could not be started: {error}",
                launch.program.display()
            ))
        }
        Err(WindowError::Close {
            spawned: mut child,
            error,
        }) => {
            // This process's copy of the node's end did not close cleanly,
            // so the connection could not be trusted to report the node's
            // death, and the node is not held by it. This process's own end
            // went with the failed window: the node reads end of stream and
            // stops cleanly. It is reaped beside, not waited for here.
            drop(
                std::thread::Builder::new()
                    .name("shekyld-reap".to_owned())
                    .spawn(move || drop(child.wait())),
            );
            return Err(format!("The node could not be started: {error}"));
        }
    };

    let pid = child.id();
    let child = Arc::new(Mutex::new(child));
    let reader = ours
        .try_clone()
        .map_err(|e| format!("The node's connection could not be read: {e}"))?;
    let (tx, life) = watch::channel(Life::Starting);
    let watched = Arc::clone(&child);
    let stderr_path = launch.stderr.clone();
    std::thread::Builder::new()
        .name("shekyld-owning".to_owned())
        .spawn(move || watch_life(reader, &watched, &stderr_path, &tx))
        .map_err(|e| format!("The node's connection could not be read: {e}"))?;
    Ok(OwnNode {
        ours,
        child,
        life,
        pid,
    })
}

/// Read the node's end of the connection for its whole life: the ready
/// line, then nothing until the end of stream that says the process is
/// gone. Then reap it and say so.
fn watch_life(mut reader: End, child: &Mutex<Child>, stderr: &Path, tx: &watch::Sender<Life>) {
    if read_ready(&mut reader).is_ok() {
        tx.send_replace(Life::Ready);
    }
    let mut rest = [0u8; 64];
    // The node writes nothing after its ready line; this returns at end of
    // stream or on an error, and both mean the connection is gone. After a
    // start that refused it returns at once.
    while matches!(reader.read(&mut rest), Ok(n) if n > 0) {}
    // The node holds its end until its process exits, so the exit is
    // already under way. The lock is taken per look and never across a
    // wait, so a force stop is never kept out by this thread.
    loop {
        let state = child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_wait();
        match state {
            Ok(None) => std::thread::sleep(REAP_POLL),
            Ok(Some(_)) | Err(_) => break,
        }
    }
    let refusal = std::fs::read_to_string(stderr)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    tx.send_replace(Life::Gone { refusal });
}

/// Windows: the node is a console program, and a GUI that starts one would
/// flash a console window without this.
#[cfg(windows)]
fn no_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NO_WINDOW` (`processthreadsapi.h`).
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_console_window(_command: &mut Command) {}
