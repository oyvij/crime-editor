//! JSON-RPC over a child's stdio — the channel a language server speaks on —
//! and the Debug Adapter Protocol over a Debug adapter's, which is a second
//! channel rather than a reuse (ADR 0021).
//!
//! `pty.rs` is this pattern with the other framing: a child, a reader thread,
//! and whatever arrived handed to the core as an event. The framing itself is
//! `lsp_server`'s — rust-analyzer's own crate, synchronous, and the reason no
//! `Content-Length` is spelled out here.
//!
//! Nothing in this file decides anything. What is said is built in `varde::lsp`
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
        // is Varde's own bug and the conversation cannot continue past it, so
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
    /// A server outliving Varde is a process the user cannot see and did not
    /// start. The protocol's own `shutdown` needs a reply nobody is left to
    /// read, so this is the honest end of it.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A Debug adapter over its stdio: the same child, reader thread and drain as
/// [`Server`], in the Debug Adapter Protocol's framing. That framing is LSP's
/// `Content-Length` header, but a DAP message has no `jsonrpc` field, so
/// `lsp_server` cannot read one and no crate frames it for a client
/// (`docs/stack.md`).
pub struct Adapter {
    child: Child,
    stdin: BufWriter<std::process::ChildStdin>,
    incoming: Receiver<String>,
    pub alive: bool,
}

impl Adapter {
    pub fn spawn(
        command: &str,
        args: &[String],
        cwd: &Path,
        log: Option<File>,
    ) -> std::io::Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // For the reason a server's does: this process's stderr is the
            // screen.
            .stderr(log.map_or_else(Stdio::null, Stdio::from))
            .spawn()?;
        let stdout = child.stdout.take().expect("a piped stdout");
        let stdin = child.stdin.take().expect("a piped stdin");
        let (sender, incoming) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            // A stream that cannot be read as frames any more ends the
            // conversation: every message after a desynchronised header is
            // garbage, and the loss reaches the core as an adapter that is gone.
            while let Some(json) = read_frame(&mut reader) {
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

    pub fn send(&mut self, json: &str) {
        let written = write!(self.stdin, "Content-Length: {}\r\n\r\n{json}", json.len())
            .and_then(|()| self.stdin.flush());
        if written.is_err() {
            self.alive = false;
        }
    }

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

impl Drop for Adapter {
    /// An adapter outliving its session holds a debugged program nobody can
    /// see. `disconnect` has had its answer by the time a session lets it go.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One message off a `Content-Length` framed stream: headers to a blank line,
/// then exactly that many bytes. `None` at the end of the stream or at
/// anything that is not a frame.
fn read_frame(reader: &mut impl std::io::BufRead) -> Option<String> {
    let mut length = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
    }
    let mut body = vec![0; length?];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

#[cfg(test)]
mod tests {
    use super::read_frame;

    /// Two frames back to back, a header the framing does not know, and a
    /// body holding a multi-byte character — the length counts bytes.
    #[test]
    fn frames_are_read_by_their_byte_length() {
        let stream = "Content-Length: 12\r\n\r\n{\"seq\":\"é\"}Content-Type: x\r\ncontent-length: 2\r\n\r\n{}";
        let mut reader = std::io::BufReader::new(stream.as_bytes());
        assert_eq!(read_frame(&mut reader).as_deref(), Some("{\"seq\":\"é\"}"));
        assert_eq!(read_frame(&mut reader).as_deref(), Some("{}"));
        assert_eq!(read_frame(&mut reader), None);
    }
}
