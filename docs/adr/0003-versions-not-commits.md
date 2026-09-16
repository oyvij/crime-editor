# An Update is a newer Version, not a newer commit

CRIME is installed as a symlink into its own checkout, so the binary and the checkout drift apart
the moment somebody builds neither. To notice the drift, CRIME has to compare what it is against
what is on disk — and there are two things it could compare.

We compare **Versions**: the number the checkout claims about itself against the Running version
the binary was compiled from. An Update exists when the checkout's is strictly newer. That number
is meaningful to a person — they can see at a glance whether they are one patch or one breaking
change behind — and reading it is one file read and one comparison, with no git operation, no
network call, and nothing that can touch the working tree.

The obvious alternative is the commit hash, and it has one genuine advantage we gave up: it cannot
be forgotten. A Version only moves when somebody moves it, so a change committed without a bump is
invisible to this feature by construction — see the version-bump rule in `AGENTS.md`, which exists
because of exactly this weakness. The hash lost anyway, on three counts. Stamping it into the binary
needs a build script or a proc macro, so the feature starts by adding build machinery. The
comparison then lives at the edge — reading the checkout's current commit is a git operation, and
the edge is where no scenario can cover it — whereas comparing two version strings is a pure
function with a fast test per branch. And a hash tells a person nothing: "you are on `3f9a1c2`, the
checkout is on `b71e004`" answers no question anybody asked.

## Consequences

Forgetting to bump is silent. Nothing fails, no test catches it, and the user is simply not told
about a change they already have. That is the accepted cost, and it is preferred to the opposite
failure: a number that moved on every commit would raise the notice constantly and teach the user
to ignore it.

The comparison is ordering, not inequality. A checkout that is *behind* the binary — an old branch
checked out — is not an Update and must not be offered as one, so this is `>` and never `!=`.

A future reader expecting a git-based updater should read this before building one. The absence of
git here is a decision, not an oversight.
