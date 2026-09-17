# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the actual
label strings used in this repo's issue tracker.

The right-hand column is the GitHub label applied to the issue. One triage label per issue; changing
state means removing the old label as the new one is added (`gh issue edit <n> --remove-label … --add-label …`).
A label that does not exist on the repo yet is created with `gh label create <name>`.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

These five are the roles `/triage` moves an issue between. They are triage states, not progress: a
finished ticket does not appear here, because finishing means *closing* the issue — see
`issue-tracker.md`.

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the corresponding label
string from this table.

Edit the right-hand column to match whatever vocabulary you actually use.
