//! F3 — turning a tree action into a shell command.
//!
//! String building only. The decision to inject or run lives in [`crate::update`],
//! and quoting belongs to `shlex` — see docs/stack.md.

use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    GoHere,
    NewFile,
    NewDirectory,
    Delete,
    SearchHere,
    BackToRoot,
    CopyPath,
}

/// A tree row, as a path relative to the workspace root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    File(PathBuf),
    Folder(PathBuf),
}

/// `mkdir -p` the parent first when the name reached into a folder that may not
/// exist yet. Directories always use `-p`, so one form covers both cases.
pub(crate) fn create(action: Action, base: &Path, path: &Path) -> String {
    match action {
        Action::NewDirectory => command("mkdir -p", path),
        _ => match path.parent() {
            Some(parent) if parent != base => {
                format!(
                    "{} && {}",
                    command("mkdir -p", parent),
                    command("touch", path)
                )
            }
            _ => command("touch", path),
        },
    }
}

pub(crate) fn command(verb: &str, path: &Path) -> String {
    format!(
        "{verb} {}",
        shlex::try_quote(&path.to_string_lossy()).expect("path has no NUL")
    )
}
