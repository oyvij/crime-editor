# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the
codebase. This repo is **single-context**: one `CONTEXT.md` and one `docs/adr/`, both at the root.

## Before exploring, read these

- **`CONTEXT.md`** at the repo root — the glossary of workspace concepts.
- **`docs/adr/`** — read ADRs that touch the area you're about to work in.

If either doesn't exist, **proceed silently**. Don't flag their absence; don't suggest creating them
upfront. The `/domain-modeling` skill creates them lazily when terms or decisions actually get
resolved.

Note that this repo already carries two documents these skills do not own but should read:
`AGENTS.md` (the working contract — architecture, code budget, test strategy) and
`docs/example-map.md` (the spec). Neither is a substitute for `CONTEXT.md`, and neither should be
rewritten by a skill.

## File structure

```
/
├── AGENTS.md          ← the working contract (not skill-owned)
├── CONTEXT.md         ← glossary, created lazily
├── docs/
│   ├── adr/           ← created lazily
│   ├── example-map.md
│   └── stack.md
└── src/
```

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a
test name), use the term as defined in `CONTEXT.md`. Don't drift to synonyms the glossary explicitly
avoids.

If the concept you need isn't in the glossary yet, that's a signal — either you're inventing
language the project doesn't use (reconsider) or there's a real gap (note it for
`/domain-modeling`).

## Flag ADR conflicts

If your output contradicts an existing ADR, surface it explicitly rather than silently overriding:

> _Contradicts ADR-0007 (event-sourced orders) — but worth reopening because…_
