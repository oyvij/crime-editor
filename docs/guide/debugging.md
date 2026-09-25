# Debugging

Varde debugs through the Debug Adapter Protocol: one Debug adapter per language, named in a
`[dap.<language>]` row rather than in Varde, the way a language server is
([configuration.md](configuration.md)). A Debug session is laid over Edit view — the editor stays
editable while the program is Paused.

## Breakpoints

A click in the gutter's leftmost column, or `Space` then `b` on the cursor's line, sets or removes
a Breakpoint. They exist with or without a session, move with their line as you edit, and the
project remembers them. Palette `b` lists every one in the Corner.

## Starting a session

A Launch configuration names the adapter, whether to `launch` or `attach`, and the arguments the
adapter is handed:

```toml
[launch.server]
adapter = "rust"
request = "launch"
args = { program = "target/debug/server" }
```

`Ctrl+Space` then `n` lists every Launch configuration in the global and the project file; the
arrows pick one and `Enter` starts it. Varde runs the protocol's start sequence and sends your
Breakpoints before the program runs.

The Rust adapter is `codelldb`. With it missing, starting a session is refused by name, and its row
under **Debug adapters** in Tools (`Ctrl+Space` then `v`) installs it.

## Paused

When the program pauses, the line it paused on is highlighted across the editor with a `→` in the
gutter, its file opened if it was not, and the Frames — the call stack — come into the Corner. The
keyboard stays where it was. Enter or a click on a Frame moves the Paused line to that call.

| Key | Does |
|---|---|
| `F9` | continue while Paused, pause while Running |
| `Ctrl+F2` | stop: a launched program is terminated, an attached one left running |

Both reach Varde from every pane, a shell's included, while a session exists; with none the
programs in your shells have them back. Ending the session gives the Corner back what it held.
