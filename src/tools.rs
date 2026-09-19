//! Tools — the palette's list of everything CRIME runs: language servers,
//! formatters, requirements and speech, as the config files name them, beside
//! every template row they do not name.

use crate::startup::{self, Config, PROGRAMS};
use crate::{lsp, State};
use std::collections::BTreeMap;

/// The groups, in the order the list draws them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub availability: Availability,
    pub origin: Origin,
}

/// What a row can offer. States of one fact rather than a flag beside an
/// option, so `Missing` cannot be read without the command that answers it and
/// the ones that offer nothing cannot be read as offering one.
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
    /// Optional where `Missing`'s is not: `stopped` is a fact about the child
    /// and stays true for an OS nothing is packaged for.
    Stopped { install: Option<String> },
    /// Not on this machine, and configuration says what installs it here.
    Missing { install: String },
    /// On this machine, and something its configuration asks the edge for is
    /// not: a `[facts.*]` table declares the name and this workspace has no
    /// answer for it, so the server would run without what it needs — which is
    /// why [`lsp::sync`] starts nothing for it. Installed is the wrong word for
    /// that, and the list is the one place the difference shows before a file
    /// is opened (R31.27). It offers no install: the command is already here,
    /// and what is missing is a directory no package manager puts in a
    /// workspace.
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
}

impl Availability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Availability::Installed => "installed",
            Availability::Partial { .. } => "partly-working",
            Availability::Stopped { .. } => "stopped",
            Availability::Missing { .. } => "missing",
            Availability::Unmet { .. } => "missing-requirement",
            Availability::Unpackaged => "no-install-command",
            Availability::Available => "available",
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
                    (true, _) => Availability::Stopped {
                        install: server.install.get(&state.os).cloned(),
                    },
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
        |_, formatter| match on_path(&formatter.command) {
            true => Availability::Installed,
            false => absent(formatter.install.get(&state.os)),
        },
    ));
    // A requirement is a search, not a command: what it reads is whether the
    // edge found an answer for it in this workspace, in the word the server
    // that needs it reads. Nothing installs one yet.
    rows.extend(group(
        Kind::Requirement,
        &state.facts,
        template.facts(),
        |fact| fact.command.clone().unwrap_or_default(),
        |name, _| match state.workspace_facts.contains_key(name) {
            true => Availability::Installed,
            false => Availability::Unmet {
                needs: name.to_string(),
            },
        },
    ));
    // `[speech]` is one table naming two programs, and the voice belongs to
    // the synthesizer: a synthesizer with no voice named cannot speak, and the
    // install that fetches one is the fix, so it reads as missing.
    let speech = &state.speech;
    let shipped = startup::speech(&template, &state.os);
    let install = (!speech.install.is_empty()).then_some(&speech.install);
    let synthesizer = match speech.command.is_empty() {
        true => (
            shipped.command.clone(),
            Availability::Available,
            Origin::Template,
        ),
        false => (
            speech.command.clone(),
            match on_path(&speech.command) && !speech.voice.is_empty() {
                true => Availability::Installed,
                false => absent(install),
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
            Availability::Available,
            Origin::Template,
        ),
        false => (
            speech.player.clone(),
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
            .filter(|(_, (command, _, _))| !command.is_empty())
            .map(|(name, (command, availability, origin))| ToolRow {
                kind: Kind::Speech,
                name: name.to_string(),
                command,
                availability,
                origin,
            }),
    );
    rows
}

/// Not on this machine: what configuration says installs it here, or the
/// admission that nothing does. Which OS applies is a lookup under the string
/// `Startup` handed in, never a branch on it (R31.22).
fn absent(install: Option<&String>) -> Availability {
    match install {
        Some(install) => Availability::Missing {
            install: install.clone(),
        },
        None => Availability::Unpackaged,
    }
}

/// One group: every configured row with its status, then every template row
/// no file names.
fn group<T: PartialEq>(
    kind: Kind,
    configured: &BTreeMap<String, T>,
    template: BTreeMap<String, T>,
    command: impl Fn(&T) -> String,
    status: impl Fn(&str, &T) -> Availability,
) -> Vec<ToolRow> {
    let mut rows: Vec<ToolRow> = configured
        .iter()
        .map(|(name, row)| ToolRow {
            kind,
            name: name.clone(),
            command: command(row),
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
                availability: Availability::Available,
                origin: Origin::Template,
            }),
    );
    rows
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
            Availability::Missing {
                install: "install-piper".to_string()
            }
        );
        state.speech.voice = "~/.crime/voices/bryce.onnx".to_string();
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
