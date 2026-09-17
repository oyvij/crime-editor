# 03 — `:update` on a binary install replaces the binary and relaunches

Status: resolved

Blocked by: 02

**What to build:** The Rebuild event branches on install kind. A known checkout still runs the
build command in the terminal. A remembered Release returns an effect naming the asset and the
checksum list; the edge downloads both with `curl` to a temporary file beside the running binary,
verifies SHA-256 with the `sha2` crate against the line naming the asset, sets the executable bit,
and renames the temporary file over the running binary's *resolved* path. Each failure comes back
as an event naming its step and raises its own notice; success comes back bare.

On success the core runs the path the existing Restart event runs — which is Quit, so an unsaved
buffer raises `unsaved-changes` and nothing exits — but with a Relaunch effect in place of Exit. The
edge executes Relaunch by `exec`ing the binary at its own path with the same arguments. The core
remembers that the binary on disk is replaced, so a second `:update` after a refusal relaunches
without fetching.

The shapes:

```
Effect::ReplaceBinary { asset: String, checksums: String }
Event::BinaryReplaced(Result<(), ReplaceFailed>)
enum ReplaceFailed { Download, NoAsset, Checksum, Replace }
Effect::Relaunch
State { replaced: bool }
```

The `no-checkout` notice becomes a "nothing to update from" notice, raised when there is neither a
checkout nor a Release. `sha2` is named in `docs/stack.md` with the reason the spec gives.

## Acceptance criteria

- [x] `:update` with a Release and no checkout returns `ReplaceBinary` and runs nothing in the terminal.
- [x] `:update` with a checkout returns the build command exactly as today.
- [x] `:update` with neither notifies and returns no effect.
- [x] The palette's `u` does what `:update` does on a binary install.
- [x] Each `ReplaceFailed` kind raises a distinct notice and leaves `replaced` false.
- [x] `BinaryReplaced(Ok)` with no unsaved buffers returns `Relaunch`.
- [x] `BinaryReplaced(Ok)` with an unsaved buffer raises `unsaved-changes`, returns no `Relaunch`,
  and sets `replaced`.
- [x] `:update` with `replaced` set returns `Relaunch` and no `ReplaceBinary`.
- [x] Updating touches nothing but the binary: no AI session, no buffer, no file written.

## Comments

- `startup::verify` holds the checksum decision in the library, where a unit test can see it: it
  finds the `sha256sum` line for the Asset's name (the last segment of its download URL), hashes
  with `sha2`, and answers `NoAsset` or `Checksum`. The edge only downloads, writes and renames.
- The binary's path is resolved once, at startup, and kept on the edge. On Linux, asking
  `current_exe` again after the rename gives `crime (deleted)`.
- A binary install needs no symlink: `install.sh` (issue 05) puts a real file at
  `~/.local/bin/crime`. The rename still targets the resolved path, so a symlink someone makes by
  hand names the new binary afterwards instead of being replaced by a copy.
- `relaunching` goes through `Event::Restart`, not `Quit`, so a modal open when the download ends
  is closed before `unsaved-changes` is shown.
- Not yet verified end to end against a real Release. The swap is covered by a `file://` test in
  `main.rs`; the relaunch `exec` is not.
