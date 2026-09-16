# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual
label strings used in this repo's issue tracker.

This tracker is file-based, so there are no labels to apply. The right-hand column is the string
written to the `Status:` line near the top of an issue file — `Status: ready-for-agent`. One status
per file; changing state means rewriting that line, not adding a second one.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

These five are the roles `/triage` moves an issue between. They are triage states, not progress: a
finished ticket does not appear here, because on a hosted tracker finishing means *closing* the
issue. This tracker is file-based and has no close, so `issue-tracker.md` defines one.

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label
string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.
