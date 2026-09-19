//! Tools — the palette's list of everything CRIME runs: language servers,
//! formatters, requirements and speech, as the config files name them, beside
//! every template row they do not name.

use crate::startup::{self, Config, ConfigError, PROGRAMS};
use crate::{lsp, State};
use std::collections::BTreeMap;
use std::path::Path;

/// The groups, in the order the list draws them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Server,
    Formatter,
    Requirement,
    Speech,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Server => "language-servers",
            Kind::Formatter => "formatters",
            Kind::Requirement => "requirements",
            Kind::Speech => "speech",
        }
    }

    /// The table a row of this kind is under in a config file. Speech is keys
    /// of one `[speech]` table rather than a table per row, so taking one
    /// appends nothing.
    fn section(self) -> Option<&'static str> {
        match self {
            Kind::Server => Some("lsp"),
            Kind::Formatter => Some("formatter"),
            Kind::Requirement => Some("facts"),
            Kind::Speech => None,
        }
    }
}

/// How a row stands against the template CRIME carries. A row that differs
/// is not refreshed: a corrected template reaches a file that already has the
/// row only by the reader reading the difference and editing their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Template,
    Differs,
    /// A row the reader wrote for a program the template does not know.
    Own,
}

#[derive(Debug)]
pub struct ToolRow {
    pub kind: Kind,
    pub name: String,
    pub command: String,
    /// What installs it on this OS: the file's, or for an available row the
    /// template's.
    pub install: Option<String>,
    pub availability: Availability,
    pub origin: Origin,
}

/// How a row stands on this machine. What installs it is the row's own
/// `install`; whether taking the row runs it is decided by this.
///
/// The two questions a reader has — is the command here, and does it work —
/// are one enum and not two fields, because they are not independent: a
/// conversation only exists for a language whose command could be spawned, so
/// read apart they would produce combinations nothing can be in, and read
/// together by every caller they would be the flag-beside-an-option this type
/// exists to avoid. `Installed` was the field that could not tell a rustup shim
/// from a server: `which` was happy, the spawn succeeded, the child died, and
/// the row went on saying the command was here (R31.25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// The command is on this machine, has what its configuration asks for, and
    /// nothing has been watched to fail — which is as much as can be said about
    /// a language no file has needed yet.
    Installed,
    /// The command is on this machine and CRIME watched its server go: a
    /// written-off conversation, from [`lsp::gone`], which is the edge's own
    /// observation rather than a probe of ours (R31.10 — nothing is spawned to
    /// find out what a row says). It offers whatever installs it, because a
    /// command that is present and does not work is exactly the row an install
    /// would fix, and refusing it as already installed is refusing the fix.
    /// `stopped` is a fact about the child, so it stays true for an OS nothing
    /// is packaged for.
    Stopped,
    /// Not on this machine, and configuration says what installs it here.
    Missing,
    /// On this machine, and something its configuration asks the edge for is
    /// not: a `[facts.*]` table declares the name and this workspace has no
    /// answer for it, so the server would run without what it needs — which is
    /// why [`lsp::sync`] starts nothing for it. Installed is the wrong word for
    /// that, and the list is the one place the difference shows before a file
    /// is opened (R31.27). It offers the fact's install, not the server's: the
    /// command is already here, and what is missing is usually machine-wide —
    /// a server on `PATH` with no classic `tsc` beside it.
    Unmet { needs: String },
    /// Not on this machine, and nothing is configured to install it for this
    /// OS. A normal row and not an error: several servers are genuinely
    /// packaged nowhere, and admitting the gap is what makes it fixable in one
    /// line of TOML (`docs/adr/0012-an-install-command-is-configuration.md`).
    Unpackaged,
    /// On this machine, running, and configuration says it answers only part of
    /// what a reader would expect of it. Declared rather than observed, because
    /// nothing CRIME can watch tells a server that answers less from a file
    /// with less wrong in it: `@vue/language-server` marks template mistakes
    /// and reports no type error at all, and its row read `installed` while
    /// half of what a reader opened the file for was silently absent. That is
    /// the state between `installed` and `missing` R31.25 has no word for, and
    /// the words are configuration's for the reason a command is — a limitation
    /// is a fact about a server, and an arm naming one is R31.1's forbidden arm.
    Partial { without: String },
    /// A template row no config file names: CRIME knows the program and does
    /// not run it, because a row that is not in a file does not run
    /// (`docs/adr/0018-the-global-config-is-the-list-of-programs.md`).
    Available,
    /// Taken, and its install reported a status other than 0. Said on the row
    /// until it is taken again, because an install that failed in a pane the
    /// reader has since scrolled past is otherwise a failure nobody sees.
    InstallFailed,
    /// Not on this machine, and the program its install starts with is not
    /// either, so taking it would write a row and run a command that fails on
    /// its first word. Named, because the fix is installing that program, and
    /// that is `install.sh`'s job, not a row's.
    NeedsInstaller { installer: String },
}

impl Availability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Availability::Installed => "installed",
            Availability::Partial { .. } => "partly-working",
            Availability::Stopped => "stopped",
            Availability::Missing => "missing",
            Availability::Unmet { .. } => "missing-requirement",
            Availability::Unpackaged => "no-install-command",
            Availability::Available => "available",
            Availability::InstallFailed => "install-failed",
            Availability::NeedsInstaller { .. } => "needs-installer",
        }
    }
}

/// The rows the config files name, and after them in each group the template
/// rows they do not. Read off the merged configuration rather than a table of
/// its own, which is what makes a project's own `[lsp.rust]` show its command
/// here and a language added to a file appear without a release (R31.21).
pub fn rows(state: &State) -> Vec<ToolRow> {
    let template = Config(PROGRAMS.parse().expect("the template parses"));
    let on_path = |command: &str| state.commands_on_path.contains(command);
    let mut rows = group(
        Kind::Server,
        &state.servers,
        template.servers(),
        |server| server.command.clone(),
        // A server that is here and missing a requirement is fixed by the
        // requirement's install, not its own.
        |server| match on_path(&server.command) {
            true => match lsp::unmet(state, server) {
                Some(needs) => state
                    .facts
                    .get(&needs)
                    .and_then(|fact| fact.install.get(&state.os).cloned()),
                None => server.install.get(&state.os).cloned(),
            },
            false => server.install.get(&state.os).cloned(),
        },
        |language, server| match on_path(&server.command) {
            // A command that is here still answers for what its configuration
            // asks the edge to find: a Vue server started without its SDK is a
            // row that reads `installed` and a server that dies on the first
            // file.
            true => match lsp::unmet(state, server) {
                Some(needs) => Availability::Unmet { needs },
                // Ahead of `Installed` and behind `Unmet`: a requirement this
                // workspace cannot meet is what the reader has to fix and names
                // itself, while `stopped` is what is left when the command is
                // here, has what it needs, and still does not work.
                None => match (lsp::written_off(state, language), &server.partial) {
                    (true, _) => Availability::Stopped,
                    // Behind `Stopped`: a server that is not running answers
                    // nothing at all, which is not "partly".
                    (false, Some(without)) => Availability::Partial {
                        without: without.clone(),
                    },
                    (false, None) => Availability::Installed,
                },
            },
            false => absent(server.install.get(&state.os)),
        },
    );
    rows.extend(group(
        Kind::Formatter,
        &state.formatters,
        template.formatters(),
        |formatter| formatter.command.clone(),
        |formatter| formatter.install.get(&state.os).cloned(),
        |_, formatter| match on_path(&formatter.command) {
            true => Availability::Installed,
            false => absent(formatter.install.get(&state.os)),
        },
    ));
    // A requirement is a search, not a command: what it reads is whether the
    // edge found an answer for it in this workspace, in the word the server
    // that needs it reads.
    rows.extend(group(
        Kind::Requirement,
        &state.facts,
        template.facts(),
        |fact| fact.command.clone().unwrap_or_default(),
        |fact| fact.install.get(&state.os).cloned(),
        |name, _| match state.workspace_facts.contains_key(name) {
            true => Availability::Installed,
            false => Availability::Unmet {
                needs: name.to_string(),
            },
        },
    ));
    // `[speech]` is one table naming two programs, and the voice belongs to
    // the synthesizer: a synthesizer with no voice on disk cannot speak, and
    // the install that fetches one is the fix, so it reads as missing.
    let speech = &state.speech;
    let shipped = startup::speech(&template, &state.os);
    let given = |install: &String| (!install.is_empty()).then(|| install.clone());
    let synthesizer = match speech.command.is_empty() {
        true => (
            shipped.command.clone(),
            given(&shipped.install),
            Availability::Available,
            Origin::Template,
        ),
        false => (
            speech.command.clone(),
            given(&speech.install),
            match on_path(&speech.command) && state.voice_installed {
                true => Availability::Installed,
                false => absent(given(&speech.install).as_ref()),
            },
            match (&speech.command, &speech.args, &speech.install)
                == (&shipped.command, &shipped.args, &shipped.install)
            {
                true => Origin::Template,
                false => Origin::Differs,
            },
        ),
    };
    let player = match speech.player.is_empty() {
        true => (
            shipped.player.clone(),
            None,
            Availability::Available,
            Origin::Template,
        ),
        false => (
            speech.player.clone(),
            None,
            match on_path(&speech.player) {
                true => Availability::Installed,
                false => Availability::Unpackaged,
            },
            match speech.player == shipped.player {
                true => Origin::Template,
                false => Origin::Differs,
            },
        ),
    };
    rows.extend(
        [("synthesizer", synthesizer), ("player", player)]
            .into_iter()
            // Neither file nor template names it for this OS.
            .filter(|(_, (command, _, _, _))| !command.is_empty())
            .map(|(name, (command, install, availability, origin))| ToolRow {
                kind: Kind::Speech,
                name: name.to_string(),
                command,
                install,
                availability,
                origin,
            }),
    );
    // Last, over whatever the row would otherwise read: the install the
    // reader asked for is the newest thing known about it.
    for row in &mut rows {
        if state.install_failed.contains(&(row.kind, row.name.clone())) {
            row.availability = Availability::InstallFailed;
        }
    }
    // Except a package manager gone missing, which outranks even that: taking
    // the row again would run an install that fails on its first word. Never
    // over a command that is here, which needs no installer.
    for row in &mut rows {
        if matches!(
            row.availability,
            Availability::Missing | Availability::Available | Availability::InstallFailed
        ) && !on_path(&row.command)
        {
            if let Some(installer) = row
                .install
                .as_deref()
                .and_then(installer)
                .filter(|installer| !on_path(installer))
            {
                row.availability = Availability::NeedsInstaller { installer };
            }
        }
    }
    rows
}

/// Not on this machine: whether configuration says what installs it here, or
/// admits that nothing does. Which OS applies is a lookup under the string
/// `Startup` handed in, never a branch on it (R31.22).
fn absent(install: Option<&String>) -> Availability {
    match install {
        Some(_) => Availability::Missing,
        None => Availability::Unpackaged,
    }
}

/// The package manager an install runs: its first word, or the one after
/// `sudo`, which only borrows another user's rights for it.
pub fn installer(install: &str) -> Option<String> {
    let mut words = shlex::Shlex::new(install);
    match words.next()? {
        sudo if sudo == "sudo" => words.next(),
        first => Some(first),
    }
}

/// One group: every configured row with its status, then every template row
/// no file names.
fn group<T: PartialEq>(
    kind: Kind,
    configured: &BTreeMap<String, T>,
    template: BTreeMap<String, T>,
    command: impl Fn(&T) -> String,
    install: impl Fn(&T) -> Option<String>,
    status: impl Fn(&str, &T) -> Availability,
) -> Vec<ToolRow> {
    let mut rows: Vec<ToolRow> = configured
        .iter()
        .map(|(name, row)| ToolRow {
            kind,
            name: name.clone(),
            command: command(row),
            install: install(row),
            availability: status(name, row),
            origin: match template.get(name) {
                Some(shipped) if shipped == row => Origin::Template,
                Some(_) => Origin::Differs,
                None => Origin::Own,
            },
        })
        .collect();
    rows.extend(
        template
            .iter()
            .filter(|(name, _)| !configured.contains_key(*name))
            .map(|(name, row)| ToolRow {
                kind,
                name: name.clone(),
                command: command(row),
                install: install(row),
                availability: Availability::Available,
                origin: Origin::Template,
            }),
    );
    rows
}

/// Where a taken row's install reports its exit status, inside CRIME's own
/// directory, which the watcher always watches.
pub const SENTINEL: &str = "install-done";

/// The install, as one line for the shell pane to run, reporting its exit
/// status in `sentinel` whatever it was. Written exactly as
/// [`crate::story::download_command`] writes a clone's, for the reasons given
/// there: the stale sentinel goes first, and the status is moved into place so
/// the watcher never sees the file before its contents. The install itself is
/// configuration's string, run as the reader wrote it.
pub fn reported(install: &str, sentinel: &Path) -> String {
    let quoted = |path: &Path| {
        shlex::try_quote(&path.to_string_lossy())
            .expect("no NUL in a path")
            .into_owned()
    };
    let writing = quoted(&sentinel.with_extension("writing"));
    let sentinel = quoted(sentinel);
    format!("rm -f {sentinel}; {install}; echo $? > {writing}; mv {writing} {sentinel}")
}

/// What the global config is read for: taking a row writes the row, and its
/// install exiting 0 writes what the row `configures`. Carried out and back
/// with the read, like the row it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    Row,
    Configures,
}

/// What the speech install exiting 0 writes into the global config, given the
/// file's text: each key `[speech]`'s own `configures` names that the table
/// does not already set, or `None` when it sets every one. A blank value is
/// not set — it is what the template ships until the install that fills it
/// in — and anything else is the reader's, which the template's never beats.
///
/// Read from this file and no other: a project's config is a stranger's, and
/// what it says the install configures is not the reader's to have written
/// into their own. Written as the row spells it: a `~` is expanded where the
/// value is read, never here. Replaced in place, so every comment and every
/// other key keeps its bytes, and the file is refused as [`take`] refuses one.
pub fn configure(global: &str) -> Result<Option<(String, Config)>, ConfigError> {
    startup::merged_config(Some(global), None)?;
    let mut file: toml_edit::DocumentMut =
        global.parse().expect("the file the merge just read parses");
    let Some(rows) = file
        .get_mut("speech")
        .and_then(toml_edit::Item::as_table_like_mut)
    else {
        return Ok(None);
    };
    let configures: Vec<(String, String)> = rows
        .get("configures")
        .and_then(toml_edit::Item::as_table_like)
        .into_iter()
        .flat_map(|keys| keys.iter())
        .filter_map(|(key, value)| Some((key.to_string(), value.as_str()?.to_string())))
        .collect();
    let mut wrote = false;
    for (key, value) in configures {
        let key = key.as_str();
        if rows.get(key).is_some_and(|set| set.as_str() != Some("")) {
            continue;
        }
        match rows.get_mut(key).and_then(toml_edit::Item::as_value_mut) {
            Some(blank) => {
                let decor = blank.decor().clone();
                *blank = value.as_str().into();
                *blank.decor_mut() = decor;
            }
            None => {
                rows.insert(key, toml_edit::value(value));
            }
        }
        wrote = true;
    }
    if !wrote {
        return Ok(None);
    }
    let text = file.to_string();
    let config = Config(startup::merged_config(Some(&text), None)?);
    Ok(Some((text, config)))
}

/// What taking a row writes into the global config, given the file's text: the
/// text with the template's row appended, and every `[facts.*]` row its values
/// name that the file does not have, or `None` when there is nothing to add —
/// the file already has the row, or it is not the template's to give.
///
/// Appended after the file's last byte, so nothing the reader wrote moves:
/// every comment and every hand edit is still where it was. The rows are cut
/// out of the template through `toml_edit`, which keeps the comment above each
/// one. A file that does not parse is refused with the fault the start would
/// report, and so is a file the append would make one CRIME refuses — two rows
/// claiming one extension — since the next start would read it.
pub fn take(global: &str, kind: Kind, name: &str) -> Result<Option<(String, Config)>, ConfigError> {
    let has = startup::merged_config(Some(global), None)?;
    let Some(section) = kind.section() else {
        return Ok(None);
    };
    let template: toml_edit::DocumentMut = PROGRAMS.parse().expect("the template parses");
    let Some(row) = template.get(section).and_then(|rows| rows.get(name)) else {
        return Ok(None);
    };
    let lacks = |section: &str, name: &str| {
        !has.get(section)
            .and_then(toml::Value::as_table)
            .is_some_and(|rows| rows.contains_key(name))
    };
    if !lacks(section, name) {
        return Ok(None);
    }
    let mut appended = toml_edit::DocumentMut::new();
    let append =
        |to: &mut toml_edit::DocumentMut, section: &str, name: &str, row: &toml_edit::Item| {
            let mut parent = toml_edit::Table::new();
            parent.set_implicit(true);
            to.entry(section)
                .or_insert(toml_edit::Item::Table(parent))
                .as_table_mut()
                .expect("a section is a table")
                .insert(name, row.clone());
        };
    append(&mut appended, section, name, row);
    // Rendered rather than read off the item, whose own text leaves out its
    // sub-tables — where `[lsp.typescript]` names both of its facts.
    let named = appended.to_string();
    for (fact, row) in template
        .get("facts")
        .and_then(toml_edit::Item::as_table)
        .into_iter()
        .flatten()
    {
        if named.contains(&format!("${{{fact}}}")) && lacks("facts", fact) {
            append(&mut appended, "facts", fact, row);
        }
    }
    let mut text = global.to_string();
    if !text.is_empty() {
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push('\n');
    }
    text.push_str(appended.to_string().trim_start());
    let config = Config(startup::merged_config(Some(&text), None)?);
    Ok(Some((text, config)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(state: &State, kind: Kind, name: &str) -> ToolRow {
        rows(state)
            .into_iter()
            .find(|row| row.kind == kind && row.name == name)
            .unwrap_or_else(|| panic!("no {} row {name}", kind.as_str()))
    }

    fn speaking() -> State {
        let mut state = State {
            os: "linux".to_string(),
            ..State::default()
        };
        state.speech.command = "piper".to_string();
        state.speech.install = "install-piper".to_string();
        state.speech.player = "aplay".to_string();
        state.commands_on_path.insert("piper".to_string());
        state
    }

    /// The synthesizer is installed only with a voice to speak in, and the
    /// install that fetches one is what the row offers until then.
    #[test]
    fn a_synthesizer_with_no_voice_is_missing_its_install() {
        let mut state = speaking();
        assert_eq!(
            row(&state, Kind::Speech, "synthesizer").availability,
            Availability::Missing
        );
        assert_eq!(
            row(&state, Kind::Speech, "synthesizer").install.as_deref(),
            Some("install-piper")
        );
        state.voice_installed = true;
        assert_eq!(
            row(&state, Kind::Speech, "synthesizer").availability,
            Availability::Installed
        );
    }

    /// Nothing installs a player, so one that is not here admits it.
    #[test]
    fn a_player_reads_off_the_path_and_offers_no_install() {
        let mut state = speaking();
        assert_eq!(
            row(&state, Kind::Speech, "player").availability,
            Availability::Unpackaged
        );
        state.commands_on_path.insert("aplay".to_string());
        assert_eq!(
            row(&state, Kind::Speech, "player").availability,
            Availability::Installed
        );
    }

    /// A requirement is read off what the edge found in this workspace, not
    /// off `PATH`: its command is where the search ends, not the answer.
    #[test]
    fn a_requirement_is_installed_when_the_edge_found_it() {
        let mut state = State::default();
        let template = Config(PROGRAMS.parse().expect("the template parses"));
        state.facts = template.facts();
        state.commands_on_path.insert("tsc".to_string());
        assert_eq!(
            row(&state, Kind::Requirement, "typescript_sdk").availability,
            Availability::Unmet {
                needs: "typescript_sdk".to_string()
            }
        );
        state
            .workspace_facts
            .insert("typescript_sdk".to_string(), "/sdk".to_string());
        assert_eq!(
            row(&state, Kind::Requirement, "typescript_sdk").availability,
            Availability::Installed
        );
    }

    /// The template's TypeScript SDK is one global install on every OS, which
    /// is what a server here without it offers.
    #[test]
    fn the_typescript_sdk_installs_everywhere() {
        let template = Config(PROGRAMS.parse().expect("the template parses"));
        let sdk = &template.facts()["typescript_sdk"];
        for os in ["macos", "linux", "windows"] {
            assert_eq!(
                sdk.install.get(os).map(String::as_str),
                Some("npm install -g typescript")
            );
        }
    }

    /// The append is after the file's last byte, so every comment and every
    /// table the reader wrote is kept exactly, and the row comes with the
    /// requirement its values name.
    #[test]
    fn taking_a_row_keeps_the_file_byte_for_byte_and_brings_its_requirement() {
        let global = "# mine, and hand-aligned\n[lsp.rust]\ncommand   = \"ra\"  # pinned\nextensions = [\"rs\"]";
        let (text, config) = take(global, Kind::Server, "vue")
            .expect("parses")
            .expect("the file lacks vue");
        assert!(text.starts_with(&format!("{global}\n\n")), "{text}");
        assert!(config.servers().contains_key("vue"));
        assert!(config.facts().contains_key("typescript_sdk"));
        assert_eq!(config.servers()["rust"].command, "ra");
        assert!(text.ends_with('\n'), "{text}");
    }

    /// A requirement the file already has is not written twice, and a row it
    /// already has is not written at all.
    #[test]
    fn taking_writes_only_what_the_file_lacks() {
        let (with_fact, _) = take("", Kind::Requirement, "typescript_sdk")
            .expect("parses")
            .expect("an empty file lacks it");
        let (text, _) = take(&with_fact, Kind::Server, "vue")
            .expect("parses")
            .expect("the file lacks vue");
        assert_eq!(text.matches("[facts.typescript_sdk]").count(), 1, "{text}");
        assert!(take(&text, Kind::Server, "vue").expect("parses").is_none());
    }

    /// A row with sub-tables keeps them together, under its own header, and
    /// not interleaved with the tables of the file it lands in.
    #[test]
    fn a_row_with_sub_tables_is_appended_whole() {
        let global = "[lsp.rust]\ncommand = \"ra\"\nextensions = [\"rs\"]\n\n[formatter.rust]\ncommand = \"rustfmt\"\nextensions = [\"rs\"]\n";
        let (text, config) = take(global, Kind::Server, "typescript")
            .expect("parses")
            .expect("the file lacks typescript");
        assert!(text.starts_with(global), "{text}");
        assert!(config.facts().contains_key("typescript_sdk"), "{text}");
        assert!(
            config.facts().contains_key("vue_typescript_plugin"),
            "{text}"
        );
        let template = Config(PROGRAMS.parse().expect("the template parses"));
        assert_eq!(
            config.servers()["typescript"],
            template.servers()["typescript"]
        );
    }

    /// A row the append would make a file CRIME refuses — two rows claiming
    /// one extension — is refused before anything is written.
    #[test]
    fn an_append_that_would_break_the_file_is_refused() {
        let global = "[lsp.mine]\ncommand = \"mine\"\nextensions = [\"go\"]\n";
        assert!(matches!(
            take(global, Kind::Server, "go"),
            Err(ConfigError {
                fault: startup::ConfigFault::ClaimedTwice { .. },
                ..
            })
        ));
    }

    /// The blank the template ships is filled where it stands, `~` and all,
    /// and nothing else in the file moves.
    #[test]
    fn configuring_fills_a_blank_in_place_and_keeps_the_rest() {
        let global = "# mine\n[speech]\ncommand = \"piper\"  # pinned\nvoice = \"\"   # blank\nconfigures.voice = \"~/.crime/voices/v.onnx\"\n\n[lsp.rust]\ncommand = \"ra\"\nextensions = [\"rs\"]\n";
        let (text, config) = configure(global)
            .expect("parses")
            .expect("the voice is blank");
        assert_eq!(
            text,
            global.replace("voice = \"\"", "voice = \"~/.crime/voices/v.onnx\"")
        );
        assert_eq!(
            config.get("speech.voice").as_deref(),
            Some("~/.crime/voices/v.onnx")
        );
    }

    /// A key the file does not name at all is added; one the reader set is
    /// never overwritten, and a file that says nothing is configured writes
    /// nothing.
    #[test]
    fn configuring_adds_what_is_absent_and_never_beats_the_reader() {
        let absent = "[speech]\ncommand = \"piper\"\nconfigures.voice = \"~/v\"\n";
        let (text, _) = configure(absent)
            .expect("parses")
            .expect("the voice is absent");
        assert_eq!(text, format!("{absent}voice = \"~/v\"\n"));
        let chosen = "[speech]\nvoice = \"/mine.onnx\"\nconfigures.voice = \"~/v\"\n";
        assert!(configure(chosen).expect("parses").is_none());
        let unsaid = "[speech]\nvoice = \"\"\n";
        assert!(configure(unsaid).expect("parses").is_none());
    }

    #[test]
    fn configuring_a_file_that_does_not_parse_is_refused() {
        assert!(configure("[speech").is_err());
    }

    #[test]
    fn the_installer_is_the_first_word_or_the_one_after_sudo() {
        assert_eq!(installer("sudo apt install clangd").as_deref(), Some("apt"));
        assert_eq!(installer("npm install -g pyright").as_deref(), Some("npm"));
        assert_eq!(installer("").as_deref(), None);
    }

    /// A template row whose command is here reads available whatever its
    /// install starts with, and one whose install failed still says when
    /// its package manager has gone.
    #[test]
    fn only_a_command_that_is_not_here_needs_its_installer() {
        let mut state = State {
            os: "linux".to_string(),
            ..State::default()
        };
        state.commands_on_path.insert("gopls".to_string());
        assert_eq!(
            row(&state, Kind::Server, "go").availability,
            Availability::Available
        );
        state.commands_on_path.clear();
        state
            .install_failed
            .insert((Kind::Server, "go".to_string()));
        assert_eq!(
            row(&state, Kind::Server, "go").availability,
            Availability::NeedsInstaller {
                installer: "go".to_string()
            }
        );
    }

    /// A row the template has never heard of is the reader's own, which is
    /// neither the template's nor a difference from it.
    #[test]
    fn a_row_the_template_does_not_know_is_the_readers_own() {
        let mut state = State::default();
        state.formatters.insert(
            "ruby".to_string(),
            startup::Formatter {
                command: "rubocop".to_string(),
                args: vec![],
                install: BTreeMap::new(),
                extensions: vec!["rb".to_string()],
            },
        );
        assert_eq!(row(&state, Kind::Formatter, "ruby").origin, Origin::Own);
    }
}
