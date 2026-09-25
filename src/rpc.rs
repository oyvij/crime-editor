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
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

/// How long a server-reached Debug adapter has to start listening. Generous,
/// since an adapter unpacking its own runtime on first start is slow, and one
/// that exits instead ends the wait at once.
const CONNECT: std::time::Duration = std::time::Duration::from_secs(10);

pub struct Server {
    child: Child,
    outgoing: Sender<String>,
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
        // Writing is on a thread of its own, because a pipe holds 64 KB and a
        // server busy with the last file is not reading: a `didOpen` carrying
        // a large one would stop the screen until it did (ADR 0023). Nothing
        // here is dropped quietly either. A message the protocol cannot carry
        // is Varde's own bug and the conversation cannot continue past it, so
        // it ends the way a closed stdin does: the thread exits, and the next
        // send finds the channel closed.
        let (outgoing, unsent) = channel::<String>();
        std::thread::spawn(move || {
            let mut stdin = BufWriter::new(stdin);
            for json in unsent {
                let written = serde_json::from_str::<Message>(&json)
                    .ok()
                    .map(|message| message.write(&mut stdin).and_then(|()| stdin.flush()));
                if !matches!(written, Some(Ok(()))) {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            outgoing,
            incoming,
            alive: true,
        })
    }

    /// One message the core built, handed to the writer thread. A server that
    /// would not take the last one is a server that is gone, which the caller
    /// learns from `alive` — one message later than the failure, never silently.
    pub fn send(&mut self, json: String) {
        if self.outgoing.send(json).is_err() {
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

/// A Debug adapter over its stdio or a TCP connection to it: the same child,
/// reader thread and drain as [`Server`], in the Debug Adapter Protocol's
/// framing. That framing is LSP's `Content-Length` header, but a DAP message
/// has no `jsonrpc` field, so `lsp_server` cannot read one and no crate frames
/// it for a client (`docs/stack.md`).
pub struct Adapter {
    /// `None` for an adapter a language server hosts: the server owns that
    /// process, and only the connection is Varde's.
    child: Option<Child>,
    outgoing: Sender<String>,
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
        Ok(Self::over(Some(child), stdout, Box::new(stdin)))
    }

    /// An adapter that listens rather than reading its stdin: spawned with
    /// `port` already in its arguments, then connected to on it. Its stdout is
    /// talk rather than protocol, so it goes where its stderr does.
    ///
    /// The spawn is here, so a command that is not there is said at once; the
    /// connection is made on a thread and arrives on the receiver, because an
    /// adapter unpacking its own runtime on a first start can take seconds to
    /// listen and the screen must not stop for it. Until it arrives nothing
    /// can be sent, and nothing is: the core is told of the adapter only then.
    pub fn connect(
        command: &str,
        args: &[String],
        cwd: &Path,
        log: Option<File>,
        port: u16,
    ) -> std::io::Result<Receiver<std::io::Result<Self>>> {
        let stdout = match &log {
            Some(file) => Stdio::from(file.try_clone()?),
            None => Stdio::null(),
        };
        let child = Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(log.map_or_else(Stdio::null, Stdio::from))
            .spawn()?;
        Ok(Self::dial(port, Some(child)))
    }

    /// The connection to an adapter listening on `port`, made on a thread for
    /// the reason [`Adapter::connect`]'s is, and given up on early if `child`
    /// — the adapter's process, where Varde spawned it — exits first.
    pub fn dial(port: u16, mut child: Option<Child>) -> Receiver<std::io::Result<Self>> {
        let (sender, connected) = channel();
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            // `localhost` rather than one address: an adapter may listen on
            // either loopback, and this tries every one the name resolves to.
            let stream = loop {
                match TcpStream::connect(("localhost", port)) {
                    Ok(stream) => break Ok(stream),
                    Err(error)
                        if started.elapsed() > CONNECT
                            || child
                                .as_mut()
                                .is_some_and(|child| !matches!(child.try_wait(), Ok(None))) =>
                    {
                        break Err(error)
                    }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
                }
            };
            let adapter = stream.and_then(|stream| Ok((stream.try_clone()?, stream)));
            // Nobody left to receive means the session was stopped while
            // this waited, and the adapter is dropped with the message, which
            // kills it. Killed and reaped here on a failure, since there is no
            // `Self` whose `Drop` would.
            let _ = sender.send(match adapter {
                Ok((reader, writer)) => Ok(Self::over(child, reader, Box::new(writer))),
                Err(error) => {
                    if let Some(child) = child.as_mut() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    Err(error)
                }
            });
        });
        connected
    }

    fn over(
        child: Option<Child>,
        reader: impl std::io::Read + Send + 'static,
        writer: Box<dyn Write + Send>,
    ) -> Self {
        let (sender, incoming) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader);
            // A stream that cannot be read as frames any more ends the
            // conversation: every message after a desynchronised header is
            // garbage, and the loss reaches the core as an adapter that is gone.
            while let Some(json) = read_frame(&mut reader) {
                if sender.send(json).is_err() {
                    break;
                }
            }
        });
        // Off the loop for the reason a server's writing is.
        let (outgoing, unsent) = channel::<String>();
        std::thread::spawn(move || {
            let mut writer = BufWriter::new(writer);
            for json in unsent {
                let written = write!(writer, "Content-Length: {}\r\n\r\n{json}", json.len())
                    .and_then(|()| writer.flush());
                if written.is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            outgoing,
            incoming,
            alive: true,
        }
    }

    pub fn send(&mut self, json: String) {
        if self.outgoing.send(json).is_err() {
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
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
    use super::{read_frame, Adapter, Server};
    use std::sync::mpsc::channel;
    use std::time::Duration;

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

    /// Bigger than a pipe's buffer, so writing it on the sender's thread
    /// waits for a reader.
    fn large(seq: usize) -> String {
        format!("{{\"seq\":{seq},\"text\":\"{}\"}}", "x".repeat(1 << 20))
    }

    fn notification(text: &str) -> String {
        format!("{{\"jsonrpc\":\"2.0\",\"method\":\"x\",\"params\":{{\"text\":\"{text}\"}}}}")
    }

    #[test]
    fn an_adapter_not_reading_holds_nobody_up_and_gets_every_message_in_order() {
        let (reader, writer) = std::io::pipe().unwrap();
        let mut adapter = Adapter::over(None, std::io::empty(), Box::new(writer));
        let (done, sent) = channel();
        std::thread::spawn(move || {
            for seq in 0..3 {
                adapter.send(large(seq));
            }
            let _ = done.send(adapter);
        });
        let adapter = sent
            .recv_timeout(Duration::from_secs(2))
            .expect("sending waited for the adapter to read");
        assert!(adapter.alive);
        let mut reader = std::io::BufReader::new(reader);
        for seq in 0..3 {
            assert_eq!(read_frame(&mut reader), Some(large(seq)));
        }
    }

    #[test]
    fn an_adapter_that_stops_reading_is_reported_gone() {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let mut adapter = Adapter::over(None, std::io::empty(), Box::new(writer));
        for _ in 0..200 {
            adapter.send("{}".to_string());
            if !adapter.alive {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("a closed connection was never reported");
    }

    #[test]
    fn a_server_not_reading_holds_nobody_up() {
        let mut server =
            Server::spawn("sleep", &["30".to_string()], &std::env::temp_dir(), None).unwrap();
        let json = notification(&"x".repeat(1 << 20));
        let (done, sent) = channel();
        std::thread::spawn(move || {
            server.send(json);
            let _ = done.send(server);
        });
        let server = sent
            .recv_timeout(Duration::from_secs(2))
            .expect("sending waited for the server to read");
        assert!(server.alive);
    }

    #[test]
    fn a_server_sent_what_the_protocol_cannot_carry_is_reported_gone() {
        let mut server =
            Server::spawn("sleep", &["30".to_string()], &std::env::temp_dir(), None).unwrap();
        for _ in 0..200 {
            server.send("not a message".to_string());
            if !server.alive {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("a writer that stopped was never reported");
    }

    /// `cat` says back what it is told, so what the reader thread hands over
    /// is what reached the server, in the order it got there.
    #[test]
    fn a_server_gets_every_message_in_the_order_it_was_sent() {
        let mut server = Server::spawn("cat", &[], &std::env::temp_dir(), None).unwrap();
        for text in ["one", "two", "three"] {
            server.send(notification(text));
        }
        let mut arrived = Vec::new();
        for _ in 0..200 {
            arrived.extend(server.drain());
            if arrived.len() == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let texts: Vec<serde_json::Value> = arrived
            .iter()
            .map(|json| {
                serde_json::from_str::<serde_json::Value>(json).unwrap()["params"]["text"].clone()
            })
            .collect();
        assert_eq!(texts, ["one", "two", "three"]);
    }
}
