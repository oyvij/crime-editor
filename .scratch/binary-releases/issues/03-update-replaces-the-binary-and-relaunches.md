# 03 — `:update` on a binary install replaces the binary and relaunches

Status: ready-for-agent

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

- [ ] `:update` with a Release and no checkout returns `ReplaceBinary` and runs nothing in the terminal.
- [ ] `:update` with a checkout returns the build command exactly as today.
- [ ] `:update` with neither notifies and returns no effect.
- [ ] The palette's `u` does what `:update` does on a binary install.
- [ ] Each `ReplaceFailed` kind raises a distinct notice and leaves `replaced` false.
- [ ] `BinaryReplaced(Ok)` with no unsaved buffers returns `Relaunch`.
- [ ] `BinaryReplaced(Ok)` with an unsaved buffer raises `unsaved-changes`, returns no `Relaunch`,
  and sets `replaced`.
- [ ] `:update` with `replaced` set returns `Relaunch` and no `ReplaceBinary`.
- [ ] Updating touches nothing but the binary: no AI session, no buffer, no file written.
