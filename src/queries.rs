//! What Varde answers when the child asks its terminal a question — with an
//! escape sequence, or by reading the environment it was started in.

/// The terminal identity a child in a hosted pane is given: a value to set, or
/// `None` to remove whatever the host exported. A child's behaviour must be a
/// fixed target rather than a function of which terminal Varde was launched
/// from, and `TERM` was only the variable most programs read.
///
/// Varde sets the two it can answer for and removes the rest rather than
/// inventing values. `TERM_PROGRAM` unset is what xterm and Alacritty give a
/// child too, so a program that sniffs it takes its generic path; a name Varde
/// made up is a name someone will branch on. The version query below does name
/// Varde, and the difference is what silence means on each channel: an unset
/// variable is an answer a real terminal gives, an unanswered query is not.
/// `COLORTERM` stays set because it is not an identity variable — it advertises
/// 24-bit colour, and Varde passes
/// 24-bit colour through: vt100 carries `Color::Rgb` and `ui::convert` hands it
/// to ratatui unchanged, so removing it would drop a CLI to 256 colours for
/// nothing. What the eye finally sees is the host terminal's business, as it is
/// for every colour Varde draws.
///
/// A capability database is not an identity, so `TERMINFO_DIRS` is left alone:
/// on a Nix or Homebrew ncurses it is how a terminfo entry is found at all, and
/// a child that cannot find one is broken rather than differently dressed.
/// `TERMINFO` goes, because it points at the host emulator's own entries and
/// ncurses falls back to its compiled-in path without it.
///
/// The removals are a list of terminal emulators, and lists go stale. That is
/// acceptable *here* and nowhere else in Varde: emulators are a small,
/// slow-moving, well-known set, a missed marker costs a cosmetic difference in
/// one CLI rather than a broken feature, and no Varde behaviour depends on the
/// list being complete. It is not the provider-specific branching AGENTS.md
/// forbids — nothing here names a CLI, and nothing here is a code path.
pub const CHILD_ENV: &[(&str, Option<&str>)] = &[
    ("TERM", Some("xterm-256color")),
    ("COLORTERM", Some("truecolor")),
    ("TERM_PROGRAM", None),
    ("TERM_PROGRAM_VERSION", None),
    ("TERM_SESSION_ID", None),
    ("TERMINAL_EMULATOR", None),
    ("TERMINFO", None),
    ("TERMCAP", None),
    ("LC_TERMINAL", None),
    ("LC_TERMINAL_VERSION", None),
    ("XTERM_VERSION", None),
    ("XTERM_SHELL", None),
    ("XTERM_LOCALE", None),
    ("WINDOWID", None),
    ("KITTY_WINDOW_ID", None),
    ("KITTY_PID", None),
    ("KITTY_LISTEN_ON", None),
    ("KITTY_INSTALLATION_DIR", None),
    ("KITTY_PUBLIC_KEY", None),
    ("KITTY_SHELL_INTEGRATION", None),
    ("ITERM_SESSION_ID", None),
    ("ITERM_PROFILE", None),
    ("ITERM2_SQUELCH_MARK", None),
    ("WEZTERM_PANE", None),
    ("WEZTERM_EXECUTABLE", None),
    ("WEZTERM_EXECUTABLE_DIR", None),
    ("WEZTERM_CONFIG_FILE", None),
    ("WEZTERM_CONFIG_DIR", None),
    ("WEZTERM_UNIX_SOCKET", None),
    ("ALACRITTY_WINDOW_ID", None),
    ("ALACRITTY_SOCKET", None),
    ("ALACRITTY_LOG", None),
    ("GHOSTTY_BIN_DIR", None),
    ("GHOSTTY_RESOURCES_DIR", None),
    ("GHOSTTY_SHELL_FEATURES", None),
    ("VTE_VERSION", None),
    ("TERMUX_VERSION", None),
    ("KONSOLE_VERSION", None),
    ("KONSOLE_PROFILE_NAME", None),
    ("KONSOLE_DBUS_SESSION", None),
    ("KONSOLE_DBUS_SERVICE", None),
    ("KONSOLE_DBUS_WINDOW", None),
    ("WT_SESSION", None),
    ("WT_PROFILE_ID", None),
    // A multiplexer is a terminal too, and its socket is inherited: a child that
    // finds `TMUX` set will speak tmux's passthrough sequences at Varde, which
    // hosts the pane itself and cannot read them.
    ("TMUX", None),
    ("TMUX_PANE", None),
    ("STY", None),
    ("ZELLIJ", None),
    ("ZELLIJ_SESSION_NAME", None),
    ("ZELLIJ_PANE_ID", None),
];

/// The bytes the child reads as the answer to the sequence it just printed,
/// or nothing when the sequence asked Varde nothing.
pub fn reply(
    intermediate: Option<u8>,
    params: &[&[u16]],
    final_byte: char,
    cursor: (u16, u16),
) -> Option<Vec<u8>> {
    let first = params.first().and_then(|p| p.first().copied());
    match (intermediate, final_byte, first) {
        // The cursor is a row and a column, counted from one, in the child's
        // own grid — the terminal model reports it counted from zero.
        (None, 'n', Some(6)) => {
            let (row, column) = (cursor.0 + 1, cursor.1 + 1);
            Some(format!("\x1b[{row};{column}R").into_bytes())
        }
        // Its DEC-private twin (DECXCPR), which is answered in kind: the program
        // asking it reads a reply carrying the same `?`.
        (Some(b'?'), 'n', Some(6)) => {
            let (row, column) = (cursor.0 + 1, cursor.1 + 1);
            Some(format!("\x1b[?{row};{column}R").into_bytes())
        }
        // "Are you working?" — the one query with a single correct answer and no
        // identity in it. A program that hears nothing concludes the terminal is
        // wedged.
        (None, 'n', Some(5)) => Some(b"\x1b[0n".to_vec()),
        // What terminal this is, by name and version (XTVERSION). Varde names
        // itself because that is the only answer that adds no claim to the ones
        // below; naming an xterm version would invite the `modifyOtherKeys` they
        // refuse. Argued in ADR 0004, beside the same reasoning about `TERM`.
        (Some(b'>'), 'q', None | Some(0)) => {
            Some(format!("\x1bP>|VARDE({})\x1b\\", env!("CARGO_PKG_VERSION")).into_bytes())
        }
        // A VT220 that speaks ANSI colour: everything Varde's terminal model
        // actually implements, and nothing — no sixel, no DEC locator — that it
        // would then be asked for.
        (None, 'c', None | Some(0)) => Some(b"\x1b[?62;22c".to_vec()),
        // The same VT220, with a deliberately low firmware level. Programs gate
        // features on this number — modifier reporting among them — and Varde
        // must not be mistaken for a recent xterm whose extensions it refuses.
        (Some(b'>'), 'c', None | Some(0)) => Some(b"\x1b[>1;10;0c".to_vec()),
        // Every modifier-reporting resource is off, said out loud. An unanswered
        // query leaves the program to guess, and a program that guesses disables
        // the bindings it cannot be sure of — which is how mode cycling came to
        // be off rather than merely unreachable.
        // A resource xterm does not define gets nothing: refusing a query is the
        // point, and asserting a value for a resource Varde has never heard of is
        // not. An omitted one is the first, as it is for the attributes queries.
        (Some(b'?'), 'm', resource @ (None | Some(0 | 1 | 2 | 4))) => {
            let resource = resource.unwrap_or(0);
            Some(format!("\x1b[>{resource};0m").into_bytes())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{reply, CHILD_ENV};

    fn answer(i1: Option<u8>, params: &[&[u16]], final_byte: char, cursor: (u16, u16)) -> String {
        String::from_utf8(reply(i1, params, final_byte, cursor).expect("a reply")).unwrap()
    }

    #[test]
    fn a_cursor_position_query_is_answered_with_the_cursor() {
        assert_eq!(answer(None, &[&[6]], 'n', (11, 41)), "\x1b[12;42R");
    }

    // Its DEC-private twin, which answers in kind: a program asking `CSI ? 6 n`
    // is reading a reply with the same `?` on it.
    #[test]
    fn the_private_cursor_position_query_is_answered_privately() {
        assert_eq!(answer(Some(b'?'), &[&[6]], 'n', (11, 41)), "\x1b[?12;42R");
    }

    #[test]
    fn a_primary_device_attributes_query_is_answered_with_an_identity() {
        assert_eq!(answer(None, &[], 'c', (0, 0)), "\x1b[?62;22c");
        assert_eq!(answer(None, &[&[0]], 'c', (0, 0)), "\x1b[?62;22c");
    }

    #[test]
    fn a_secondary_device_attributes_query_is_answered() {
        assert_eq!(answer(Some(b'>'), &[], 'c', (0, 0)), "\x1b[>1;10;0c");
        assert_eq!(answer(Some(b'>'), &[&[0]], 'c', (0, 0)), "\x1b[>1;10;0c");
    }

    #[test]
    fn a_status_query_is_answered_that_the_terminal_is_working() {
        assert_eq!(answer(None, &[&[5]], 'n', (0, 0)), "\x1b[0n");
    }

    // The delimiters are pinned literally rather than rebuilt with the code's own
    // expression, which could not fail: it is a DCS, and the version is the
    // binary's rather than a number someone typed twice.
    #[test]
    fn a_version_query_is_answered_with_vardes_own_name_and_version() {
        for params in [&[&[0u16][..]][..], &[]] {
            let answer = answer(Some(b'>'), params, 'q', (0, 0));
            assert_eq!(
                answer,
                format!("\x1bP>|VARDE({})\x1b\\", env!("CARGO_PKG_VERSION"))
            );
            assert!(answer.starts_with("\x1bP>|VARDE("), "{answer:?}");
            assert!(answer.ends_with(")\x1b\\"), "{answer:?}");
        }
    }

    #[test]
    fn a_modifier_reporting_query_is_answered_with_a_refusal() {
        assert_eq!(answer(Some(b'?'), &[&[4]], 'm', (0, 0)), "\x1b[>4;0m");
        assert_eq!(answer(Some(b'?'), &[&[1]], 'm', (0, 0)), "\x1b[>1;0m");
    }

    // The kitty keyboard protocol defines no negative reply, so its own detection
    // procedure pairs the query with a primary device attributes request and
    // reads the attributes answer as "unsupported". That answer — which Varde now
    // gives — is the refusal. Replying `CSI ? 0 u` would instead claim support
    // for a protocol Varde does not implement, and the program would then read
    // Varde's legacy key bytes under kitty's rules.
    // An omitted resource is the first one, as it is for the attributes queries.
    // A resource number xterm does not define gets nothing: refusing a query is
    // the point, asserting a value for a resource Varde has never heard of is not.
    #[test]
    fn a_modifier_reporting_query_without_a_resource_refuses_the_first_one() {
        assert_eq!(answer(Some(b'?'), &[], 'm', (0, 0)), "\x1b[>0;0m");
        assert!(reply(Some(b'?'), &[&[99]], 'm', (0, 0)).is_none());
    }

    #[test]
    fn the_kitty_keyboard_query_is_refused_by_answering_what_it_is_paired_with() {
        assert!(reply(Some(b'?'), &[], 'u', (0, 0)).is_none());
    }

    #[test]
    fn a_sequence_that_is_not_a_query_yields_no_reply() {
        // Copying to the printer, pushing keyboard flags, saving a private mode:
        // the terminal model does not implement them either, and none is a
        // question. A reply to any of them is text in the program's prompt.
        assert!(reply(None, &[&[4]], 'i', (3, 7)).is_none());
        assert!(reply(Some(b'>'), &[&[1]], 'u', (3, 7)).is_none());
        assert!(reply(Some(b'?'), &[&[1049]], 's', (3, 7)).is_none());
        // Both `q` sequences that are orders rather than questions: loading the
        // LEDs, and setting the cursor shape — whose intermediate is a space.
        assert!(reply(None, &[&[1]], 'q', (3, 7)).is_none());
        assert!(reply(Some(b' '), &[&[2]], 'q', (3, 7)).is_none());
        // Only version 0 of the version query is defined, and only the ANSI form
        // of the status one: a parameter Varde has never heard of gets nothing,
        // as it does for the modifier resources.
        assert!(reply(Some(b'>'), &[&[1]], 'q', (3, 7)).is_none());
        assert!(reply(Some(b'?'), &[&[5]], 'n', (3, 7)).is_none());
    }
    // The identity Varde hands a child through its environment, the other half
    // of the identity the queries above answer in escape sequences.
    #[test]
    fn the_child_is_told_the_terminal_varde_actually_renders() {
        assert!(CHILD_ENV.contains(&("TERM", Some("xterm-256color"))));
        assert!(CHILD_ENV.contains(&("COLORTERM", Some("truecolor"))));
    }

    #[test]
    fn every_other_variable_is_removed_rather_than_given_a_made_up_value() {
        for (name, value) in CHILD_ENV {
            assert!(
                matches!(*name, "TERM" | "COLORTERM") || value.is_none(),
                "{name} claims a value Varde cannot answer for"
            );
        }
    }

    // One marker per emulator family, so deleting a family is a failure rather
    // than a silent return to leaking the host's identity.
    #[test]
    fn no_emulator_the_user_may_have_launched_varde_from_leaves_a_marker() {
        for name in [
            "TERM_PROGRAM",
            "TERM_PROGRAM_VERSION",
            "TERMINFO",
            "XTERM_VERSION",
            "KITTY_WINDOW_ID",
            "ITERM_SESSION_ID",
            "WEZTERM_PANE",
            "ALACRITTY_WINDOW_ID",
            "GHOSTTY_RESOURCES_DIR",
            "VTE_VERSION",
            "KONSOLE_VERSION",
            "WT_SESSION",
            "TMUX",
        ] {
            assert!(
                CHILD_ENV.contains(&(name, None)),
                "{name} still reaches the child"
            );
        }
    }

    #[test]
    fn no_variable_is_named_twice() {
        let mut names: Vec<_> = CHILD_ENV.iter().map(|(name, _)| *name).collect();
        let all = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all, "a variable is both set and removed");
    }
}
