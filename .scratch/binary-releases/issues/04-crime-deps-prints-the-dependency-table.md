# 04 — `crime --deps` prints the dependency table

Status: resolved

**What to build:** A library function that returns the `[lsp.*]`, `[formatter.*]` and `[speech]`
rows of the shipped defaults as records — kind, name, command, and the install command for a given
OS or none — and one flag on the binary that prints them one per line, tab-separated, and exits
before the terminal is touched. Requires no folder: `crime --deps` with nothing else.

This is what lets `install.sh` serve a machine with no source (issue 05) and it is the permanent
answer to AGENTS.md's rule that a program CRIME shells out to is installable by the script: the
binary says what it needs, so there is no second table to drift. The rows come from the same parse
of `DEFAULTS` startup already does; nothing is duplicated.

Output shape, one line per row, exactly what the script's `awk` reads today from the source:

```
lsp	rust	rust-analyzer	rustup component add rust-analyzer
formatter	go	gofmt	
speech	speech	piper	uv tool install piper-tts && …
```

The `[speech]` row's `player.<os>` is printed as a fourth kind, `player`, so the script stops
grepping the source for it.

## Acceptance criteria

- [ ] The function returns every `[lsp.*]` and `[formatter.*]` row with its `install.<os>` for
  `macos` and for `linux`, blank where the table has none — pinned against the shipped defaults.
- [ ] The `[speech]` row and its player are included.
- [ ] `crime --deps` prints them and exits 0 with no folder argument and no terminal.
- [ ] The flag is documented in `docs/install.md` beside `--list`.
