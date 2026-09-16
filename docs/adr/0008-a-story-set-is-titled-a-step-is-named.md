# A Story set is titled, a Step is named, and the step-menu's width is fixed

A reviewer opening the spine cold ("The scaffold comes down (3 steps, 3 stale)") could not tell
what the whole change was *for* without reading every Story. We added a required `title` to the
Story set — one line, above the spine, naming what the range accomplishes at large (a feature
added, a bug fixed, an improvement made) — and tightened the authoring prompt so a Story's own
`name` states its concrete subject rather than a standalone metaphor. Both are enforced by prompt
wording only, not by parse-time validation: `story.rs` already documents why schema validation
stays lenient on authored content, and no heuristic can reliably tell metaphor from a name that is
merely short.

Because `title` and each Step's new `name` are required fields, this is a breaking artifact schema
change — a Story set authored before this change will fail to parse. That is deliberate (existing
sets are not worth carrying forward under a new required field) and it is why `protocolVersion`
exists; a future schema break should bump it the same way.

We also gave the walking Steps a persistent, named menu to the left of the gutter — visible only
while `Walking::Story` (never `Walking::Remainder`, which has no names to show), listing every step
of the current Story with the current one marked. Its width is a **fixed constant, like `GUTTER`**,
not derived from the longest step name in whichever Story happens to be loaded. A per-Story width
would resize the rectangle every time a reviewer entered a different Story — the exact class of bug
the "one layout" rule (`layout.rs`) exists to prevent, where a rectangle computed at one moment is
wrong by the time a click or a frame reads it. "Every name must be readable" is instead a constraint
on the authoring prompt, the same way Story and Story-set names are.

## Considered Options

- **Elastic step-menu width**, sized to the longest step name when a Story is entered, fixed for
  the walk. Rejected: still a rectangle that moves between Stories, and `layout.rs`'s tests pin
  exact rects specifically so nothing about a pane's geometry is a runtime value.
- **Interactive step-menu** (click a step to jump to it). Rejected for now: `n`/`p` already
  navigate, and a clickable menu needs its own mouse hit-test region and a focus-target decision
  that was never part of the original ask. The menu is a passive "you are here" indicator; it reads
  `Walking` and highlights, nothing more.
