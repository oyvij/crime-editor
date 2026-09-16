//! JSON-RPC over a child's stdio — the channel a language server speaks on.
//!
//! `pty.rs` is this pattern with the other framing: a child, a reader thread,
//! and whatever arrived handed to the core as an event. The framing itself is
//! `lsp_server`'s — rust-analyzer's own crate, synchronous, and the reason no
//! `Content-Length` is spelled out here.
//!
//! Nothing in this file decides anything. What is said is built in `crime::lsp`
//! and arrives as a string; what a server says goes back as a string.

use anyhow::Result;
use lsp_server::Message;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

pub struct Server {
    child: Child,
    stdin: BufWriter<std::process::ChildStdin>,
    incoming: Receiver<String>,
    /// Whether the child is still there. Read by the loop, which tells the core
    /// the moment it is not: a conversation nobody is party to any more.
    pub alive: bool,
}

impl Server {
    pub fn spawn(command: &str, args: &[String], cwd: &Path, log: Option<File>) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // A server's log goes to its stderr, and this process's stderr is
            // the screen the TUI is drawn on: inherited, the first log line
            // paints over the workspace. A file is not the screen, so the
            // caller opens one — and the type is a `File` rather than a
            // `Stdio` so that inheriting is not something a caller can pass.
            // Without one the words are lost, which costs a diagnosis and
            // never the server.
            .stderr(log.map_or_else(Stdio::null, Stdio::from))
            .spawn()?;
        let stdout = child.stdout.take().expect("a piped stdout");
        let stdin = child.stdin.take().expect("a piped stdin");
        let (sender, incoming) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Ok(Some(message)) = Message::read(&mut reader) {
                // A message that parsed can be written back — the `expect`
                // states that rather than dropping one quietly, and a thread
                // that died takes the channel with it, so the loss reaches the
                // core as a server that is gone rather than as silence.
                let json = serde_json::to_string(&message).expect("a parsed message re-serialises");
                if sender.send(json).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin: BufWriter::new(stdin),
            incoming,
            alive: true,
        })
    }

    /// One message the core built, framed and written. A server that will not
    /// take it is a server that is gone, which the caller learns from `alive`.
    pub fn send(&mut self, json: &str) {
        // Nothing here is dropped quietly. A message the protocol cannot carry
        // is CRIME's own bug and the conversation cannot continue past it, so
        // it ends the same way a closed stdin does: the caller sees `alive`
        // go false and tells the core the server is gone.
        let written = serde_json::from_str::<Message>(json).ok().map(|message| {
            message
                .write(&mut self.stdin)
                .and_then(|()| self.stdin.flush())
        });
        if !matches!(written, Some(Ok(()))) {
            self.alive = false;
        }
    }

    /// Whatever the server has said since the last pass. Every message, in
    /// order, exactly as it arrived — what it means is the core's.
    pub fn drain(&mut self) -> Vec<String> {
        let mut arrived = Vec::new();
        loop {
            match self.incoming.try_recv() {
                Ok(json) => arrived.push(json),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.alive = false;
                    break;
                }
            }
        }
        arrived
    }
}

impl Drop for Server {
    /// A server outliving CRIME is a process the user cannot see and did not
    /// start. The protocol's own `shutdown` needs a reply nobody is left to
    /// read, so this is the honest end of it.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
