//! A pty with a terminal model in front of it. Used for both the shell pane and
//! the AI pane — they differ only in the command they run.

use anyhow::Result;
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use varde::{keys, mouse, queries};

/// A terminal is two-way: the child prints a question and reads the answer on
/// its own stdin. The parser hands every sequence it does not implement here,
/// which is where all of those questions arrive; the reply is queued rather than
/// written because the writer is not reachable from inside the parse.
#[derive(Default)]
struct Answers(Vec<u8>);

impl vt100::Callbacks for Answers {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        _i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        if let Some(reply) = queries::reply(i1, params, c, screen.cursor_position()) {
            self.0.extend_from_slice(&reply);
        }
    }
}

pub struct Pane {
    parser: vt100::Parser<Answers>,
    writer: Box<dyn Write + Send>,
    output: Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    /// The child's process id, for asking the OS where it has `cd`-ed to.
    pid: Option<u32>,
    pub alive: bool,
    /// Whether the child is ready to be typed at. Its first byte is not that:
    /// a CLI's opening bytes are cursor housekeeping, and its input line — the
    /// moment it asks for bracketed paste — comes a chunk later, so a prompt
    /// sent at the first byte went in bare and submitted at its first newline.
    /// A child that prints and never asks is taken as ready once it has gone
    /// quiet, since bare is all it can be given anyway.
    pub spoken: bool,
    printed: bool,
    /// Drains since the child last printed, capped at `QUIET`.
    quiet: u8,
}

/// Drains — 16ms polls — a child that has printed goes silent for before it
/// counts as ready without ever asking for bracketed paste.
const QUIET: u8 = 5;

impl Pane {
    /// `argv` is the program to run and its arguments, or empty for the user's
    /// login shell. The login part is the whole difference between a shell
    /// that reads the profile the user's prompt and aliases live in and one
    /// that reads nothing: a shell decides it is a login shell from its own
    /// `argv[0]` having a leading dash, which is how every terminal emulator
    /// starts one and what `new_default_prog` does — including resolving
    /// `$SHELL`, and the password database when that is unset.
    ///
    /// `argv` is handed to the pty as it stands and never through a shell: the
    /// debugged program's arguments come from a Debug adapter, which is
    /// untrusted input, and a command line is something a shell would parse.
    /// `env` is what the adapter asked to be set on top of `queries::CHILD_ENV`.
    pub fn spawn(
        argv: &[String],
        cwd: &Path,
        env: &BTreeMap<String, String>,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        // A zero-sized grid panics inside vt100, and a terminal that has not
        // reported its size yet gives us zero. Two rows, not one: wrapping a
        // column needs a row to scroll into, and on a one-row grid vt100
        // subtracts the scroll off the row it came from and underflows — a
        // panic on the second character a child prints, not on the first.
        let (rows, cols) = (rows.max(2), cols.max(1));
        let pty = NativePtySystem::default().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut builder = match argv.split_first() {
            Some((program, arguments)) => {
                let mut builder = CommandBuilder::new(program);
                builder.args(arguments);
                builder
            }
            None => CommandBuilder::new_default_prog(),
        };
        builder.cwd(cwd);
        // The child's terminal identity is Varde's, not the one Varde was launched
        // from. Inherited, a child in Kitty believes it is in Kitty and emits
        // sequences vt100 cannot read — and behaves differently on every machine.
        // The list is in `queries`, beside the identity Varde gives in escape
        // sequences, so a change to one is read next to the other.
        for (name, value) in queries::CHILD_ENV {
            match value {
                Some(value) => builder.env(name, value),
                // Only the host's own, one at a time: `env_clear` would take
                // `PATH` and `HOME` with it and leave nothing runnable.
                None => builder.env_remove(name),
            }
        }
        // After Varde's own identity, so an adapter cannot talk a child out of
        // the terminal it is actually in.
        for (name, value) in env {
            builder.env(name, value);
        }
        let mut child = pty.slave.spawn_command(builder)?;
        let pid = child.process_id();
        drop(pty.slave);

        let mut reader = pty.master.try_clone_reader()?;
        let (sender, output) = channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            while let Ok(read) = reader.read(&mut buffer) {
                if read == 0 || sender.send(buffer[..read].to_vec()).is_err() {
                    break;
                }
            }
            let _ = child.wait();
        });

        Ok(Self {
            parser: vt100::Parser::new_with_callbacks(rows, cols, 2000, Answers::default()),
            writer: pty.master.take_writer()?,
            output,
            master: pty.master,
            pid,
            alive: true,
            spoken: false,
            printed: false,
            quiet: 0,
        })
    }

    /// Feeds whatever the child has produced into the terminal model, answers
    /// any question it asked while doing so, and says whether anything arrived —
    /// the caller redraws only if it did.
    pub fn drain(&mut self) -> bool {
        let mut arrived = false;
        loop {
            match self.output.try_recv() {
                Ok(chunk) => {
                    self.printed = true;
                    self.quiet = 0;
                    arrived = true;
                    self.parser.process(&chunk);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.alive = false;
                    arrived = true;
                    break;
                }
            }
        }
        if self.printed && !arrived {
            self.quiet = (self.quiet + 1).min(QUIET);
        }
        self.spoken |= self.printed && (self.screen().bracketed_paste() || self.quiet == QUIET);
        let answers = std::mem::take(&mut self.parser.callbacks_mut().0);
        if !answers.is_empty() {
            let _ = self.writer.write_all(&answers);
            let _ = self.writer.flush();
        }
        arrived
    }

    /// The folder the child is in now — where a split of this pane starts. A
    /// shell's cwd moves with every `cd`, and only the OS knows where it went.
    /// `None` when it cannot be asked, and the caller falls back to the root.
    // ponytail: shells out to lsof on macOS; libc::proc_pidinfo if the ~100ms matters.
    pub fn cwd(&self) -> Option<PathBuf> {
        let pid = self.pid?;
        if cfg!(target_os = "macos") {
            let out = std::process::Command::new("lsof")
                .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
                .output()
                .ok()?;
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .find_map(|line| line.strip_prefix('n'))
                .map(PathBuf::from)
        } else {
            std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
        }
    }

    /// Whether a foreground job is running in the shell — the terminal's own
    /// foreground process group is somebody other than the shell — so a
    /// command pushed at it would be typed into the job.
    pub fn busy(&self) -> bool {
        match (self.master.process_group_leader(), self.pid) {
            (Some(leader), Some(pid)) => leader != pid as libc::pid_t,
            _ => false,
        }
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn send(&mut self, bytes: &[u8]) {
        // Typing means you want to see what happens, so it comes back from the
        // history first — which is what every terminal does.
        self.parser.screen_mut().set_scrollback(0);
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// One notch of the wheel through the child's own history.
    pub fn scroll(&mut self, up: bool) {
        let screen = self.parser.screen_mut();
        let at = screen.scrollback();
        screen.set_scrollback(if up { at + 3 } else { at.saturating_sub(3) });
    }

    /// Whether the pane is showing history rather than the live screen, where
    /// the child's cursor no longer belongs.
    pub fn scrolled_back(&self) -> bool {
        self.screen().scrollback() > 0
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        // A zero-sized grid panics inside vt100, and ratatui hands us zero
        // whenever a pane is too small to have an interior. Two rows for the
        // reason `spawn` gives: an eight-row screen leaves the shell one row,
        // and one row panics as surely as none does — a narrow window makes it
        // certain, because an occupied Corner squeezes the shell to a single
        // column and a single column wraps on every character.
        let (rows, cols) = (rows.max(2), cols.max(1));
        if (rows, cols) == self.parser.screen().size() {
            return;
        }
        self.parser.screen_mut().set_size(rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    /// Whether the running program asked to be told that a paste is a paste.
    pub fn bracketed_paste(&self) -> keys::Paste {
        if self.screen().bracketed_paste() {
            keys::Paste::Bracketed
        } else {
            keys::Paste::Bare
        }
    }

    /// R10.5: which mouse-report encoding the running program asked for. The
    /// UTF-8 encoding (mode 1005) is reported as none rather than as legacy:
    /// their bytes diverge past column 95, and a child told the wrong cell is
    /// worse than one that hears nothing.
    pub fn mouse_encoding(&self) -> mouse::Encoding {
        match self.screen().mouse_protocol_mode() {
            vt100::MouseProtocolMode::None => mouse::Encoding::None,
            _ => match self.screen().mouse_protocol_encoding() {
                vt100::MouseProtocolEncoding::Sgr => mouse::Encoding::Sgr,
                vt100::MouseProtocolEncoding::Default => mouse::Encoding::Legacy,
                vt100::MouseProtocolEncoding::Utf8 => mouse::Encoding::None,
            },
        }
    }
}
