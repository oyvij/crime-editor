# Review

Review view is where you read what just changed — usually what the AI in the right-hand pane
just wrote — before any of it is committed. It shows only the files git reports as changed, each
as a diff, and lets you annotate ranges of lines with `ISSUE`, `NOTE`, `SUGGESTION` or `COMMENT`.
Submitting the review writes it to a file and sends it into the AI pane, so the AI starts acting
on it the moment you confirm.

The view is for reading. Nothing in a diff can be typed over; `e` takes you to the real file when
you want to change it yourself.

## Entering Review view

Open the palette (`Ctrl+Space`, or `Ctrl+Ctrl`; `Esc Esc` from inside a hosted pane) and press
`r`. Press `e` in the palette to go back to Edit view. See [Getting around](getting-around.md) for
the palette and focus.

On entry the changed-file list replaces the file tree in the left-hand pane, the first changed
file is selected, its diff is loading in the centre, and the keyboard is on the list. You switched
here to look at changes, so it shows you one straight away.

Leaving Review view clears the diff, so the editor is back to showing the file you were editing
rather than a read-only diff of it.

## What is listed

Everything uncommitted against `HEAD`:

- modified files, whether or not the change is staged;
- untracked files;
- never anything `.gitignore` covers.

Two empty states are told apart, because they mean different things:

- the folder is **not a git repository** — the view opens and says so;
- the working tree is **clean** — a different message; there is nothing to review.

## Reading a diff

The diff is a unified diff, read-only, with the code on both sides syntax-highlighted as the real
file would be. Added and removed rows carry a muted background so highlighted tokens stay readable
on top of it.

The changed-file list takes the same keys the file tree does. Moving the selection onto a file
shows its diff — reviewing is reading, so you see what you land on — and the keyboard stays on
the list so you can keep browsing. Choosing a file with `Enter` or a click shows the same diff
and hands the keyboard to it, because commenting is the next thing you do and only the editor's
keys reach it.

| Key | In the list |
|---|---|
| `Up` `Down` | move between changed files, showing each diff |
| `Enter` | show the selected file's diff and move focus into it |
| click a row | the same, and focus follows the click |

| Key | In the diff |
|---|---|
| `j` `k` | move down and up a row |
| `l` `h` | slide the diff right and left, for a long line |
| `0` | slide back home in one keypress |
| `V` | start selecting lines |
| `c` | open the comment box on the selected lines |
| `e` | open this file in Edit view, at the real file |
| `Ctrl+P` `Ctrl+N` | jump back and forward through cursor history |
| `Ctrl+C` / `Cmd+C` | copy a dragged selection |
| `Alt+h` `Alt+j` `Alt+k` `Alt+l` | move focus between panes |

Sliding sideways is a gesture rather than something the view does for you: a diff has no column
cursor to follow, so `l` and `h` move the window explicitly, and moving up and down never slides
it on its own. Sliding right stops at the longest row rather than running on into empty space.

To copy text out of a diff, drag over it with the mouse and press `Ctrl+C` or `Cmd+C`. A drag over
the *code* selects; a drag in the *gutter* — the line-number strip — is a different gesture, below.

## Commenting

A comment covers a range of lines in one file. Comments anchor to the **new** file's line
numbers, because that is the code the AI has to change; a comment on a removed line points at
where it was.

There are two ways to pick the range:

- **Keyboard.** Put the cursor on a line in the diff, press `V`, extend with `j` and `k`, then
  press `c`.
- **Mouse.** Drag down the gutter from the first line to the last. The comment box opens when you
  let go.

Either way the comment box opens over the diff and asks for a type first.

### The four kinds

| Letter | Kind | What it means |
|---|---|---|
| `i` | `ISSUE` | Something that must change. The only kind that blocks. |
| `n` | `NOTE` | Context for the AI; not a request. |
| `s` | `SUGGESTION` | A better way you can see; the AI may take it or not. |
| `c` | `COMMENT` | Anything else worth saying about these lines. |

Only those four letters pick a type. Any other key does nothing, so a stray keystroke cannot
silently choose one for you. `Esc` here closes the box and records nothing.

### Writing the body

Once a type is picked the box is a small text editor: type the body, move with the arrows, `Alt`
with `Left`/`Right` moves a word, `Alt+Backspace` deletes a word, and a paste keeps every line it
carried. `Enter` starts a new line rather than filing the comment, so a multi-line body is
ordinary. The box's footer names the three keys that are not text:

| Key | Action |
|---|---|
| `Ctrl+S` | file the comment |
| `Esc` | discard it and close the box |
| `Ctrl+Z` | undo the last edit — a pasted stack trace or a word delete goes in one key |

A filed comment is drawn in the diff against the last line of its range, and Varde tells you it
was added. Every comment also records the revision of the file that was on screen when you made
it, so a comment still points at what you actually saw even after the AI rewrites the file.

## Submitting

Type `:submit` and press `Enter`. Before anything is sent Varde asks you to confirm, because
sending clears whatever the AI's command line is currently showing — which can be a half-written
message of yours.

| Key | In the confirmation |
|---|---|
| `Enter` or `y` | submit |
| `Esc` or `n` | decline — nothing is sent, the AI's prompt is untouched, your comments stay |

An empty review cannot be submitted; Varde refuses and tells you why.

### The verdict

The review carries one of two verdicts, decided by its contents:

- `changes-requested` — the review contains at least one `ISSUE`;
- `commented` — it contains only `NOTE`s, `SUGGESTION`s and `COMMENT`s.

### Where it goes

Confirming does three things.

1. **Writes the review** to `.varde/reviews/NNNN.json` in the project, numbered on from the last
   one. The last fifty are kept; older ones are pruned on submit. From a Bare workspace (`varde`
   with no folder) the file goes to `~/.varde/reviews/` instead, so it outlives the session — see
   [Stories](stories.md#a-bare-workspace).
2. **Sends it into the AI pane** — a summary of every comment with `file:from-to`, the type and
   the body, plus the path of the file just written — and submits the prompt. You do not have to
   press `Enter` in the AI pane. When the AI's CLI supports it, the whole review arrives as a
   single paste with its line breaks intact.
3. **Clears your comments**, ready for the next pass over whatever the AI does next.

The file looks like this:

```json
{
  "verdict": "changes-requested",
  "comments": [
    {
      "file": "src/tree.js",
      "from_line": 14,
      "to_line": 18,
      "type": "ISSUE",
      "body": "unquoted path",
      "revision": "<blob id of the file as reviewed>"
    }
  ]
}
```

A comment made while walking a Story also records which story and step it was made against; one
made in Review view records neither. The artifact is what keeps the hand-off independent of which
AI CLI you run: any CLI that can read a file can act on a review.

### When no AI session is running

If the AI pane is empty, confirming starts the configured AI command first (`[ai] command`, see
[Configuration](configuration.md)) and holds the review until that CLI has printed its prompt and
is ready for input — a CLI typed at too early drops what you sent. If the CLI exits before it is
ready, or you replace it with `:ai!` in the meantime, the queued review is dropped rather than
handed to the next session; the file on disk still has it. See [AI pane](ai-pane.md) for how
sessions are started and replaced.

## Risk in Review view

Entering Review view also measures [Risk](risk.md) for exactly the files under review, comparing
each file as it is now against the revision the diff is measured from. The tree pane's top border
shows the **delta** — how much the change moved the figure — rather than the workspace's Risk
count, and a change that made things worse is marked as such. Leaving the view puts the
workspace's count back.

If the Risk list is open, it lists the Functions in the changed files with their figures and
deltas, so "which part of this change added the risk" has an answer. No Function from outside the
change appears there. A changed file in a language the analyser does not handle contributes
nothing and is never counted as an improvement.

The Risk pane's own action starts a **Refactor loop over the reviewed files** — the same loop as
the workspace one, with a different file set, so every guarantee is the same: the tests must pass,
the figure must fall, no other metric may rise, a failed pass is put back from its snapshot,
nothing is ever committed, and the loop touches no file outside the review. Stop is the same
action in the same place. [Risk](risk.md#the-refactor-loop) has the details.

## What Review view does not do

- It does not let you edit a diff. `e` opens the real file.
- It does not show committed history — only what is uncommitted against `HEAD`.
- It does not read the AI's reply. The AI's response appears in its own pane; nothing in the
  review is marked "addressed" automatically.
- It does not keep a submitted review on screen. Once sent, the comments clear; the numbered file
  is the record.
- It never edits your `.gitignore` or any other git file.

## See also

- [Stories](stories.md) — ask the AI to narrate the change you are reviewing, and comment from
  inside a Story.
- [Risk](risk.md) — the figure on the border and the loop that pays it down.
- [AI pane](ai-pane.md) — the session a submitted review is sent to.
- [Getting around](getting-around.md) — the palette, panes and focus.
