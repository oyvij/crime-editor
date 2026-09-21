Feature: Reviewing a repository that is not on this machine

  `:story? <url>` in a Bare workspace clones a repository Varde has never seen — a Guest repo —
  into the Sidecar, and lists its branches in the same picker a local repository's branches are
  listed in. The clone runs as the user's own `git` in the shell pane, so their SSH config, their
  per-host key and their agent all work without being configured twice, progress is visible, and a
  passphrase or a host-key prompt is answerable (`docs/adr/0015-a-clone-is-the-users-own-git.md`).

  Everything after the clone is `git2` like every other read in Varde. Only the clone shells out,
  and it shells out because it is the one operation that has to authenticate as the user.

  Background:
    Given the workspace root is "/home/me/projects/theirs"
    And the workspace folder holds the file "README.md"

  Scenario: A URL clones the repository into the Sidecar and lists its branches
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name           | kind   | seconds |
      | main           | local  | 200     |
      | origin/feature | remote | 300     |
    When I run ":story? git@github.com:them/theirs.git"
    Then the terminal has cloned "git@github.com:them/theirs.git" into the Sidecar
    And the story view state is "cloning"
    When the clone finishes with exit status "0"
    Then the picker lists:
      | feature |
      | main    |
    And the story view state is "no-stories"

  # The Guest repo is in the Sidecar, which is not the workspace: the folder Varde was started in
  # is what the tree is, and a foreign repository is never part of it.

  Scenario: The Guest repo is not in the file tree
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    Then the file tree shows "README.md"
    And the file tree does not show "theirs"

  Scenario: A URL in a project workspace is refused, and nothing is cloned
    Given Varde started in the project
    And git is installed
    When I run ":story? git@github.com:them/theirs.git"
    Then the story view state is "guest-needs-bare-workspace"
    And the modal is "none"
    And no command has been executed

  Scenario: A URL with no git on the machine is refused
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is not installed
    When I run ":story? git@github.com:them/theirs.git"
    Then the story view state is "git-not-installed"
    And the modal is "none"
    And no command has been executed

  # The sentinel carries the exit status rather than the fact of finishing, which is what makes a
  # clone that failed a refusal instead of a wait nobody can end.

  Scenario: A clone that failed is refused with its exit status, not silence
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    When I run ":story? git@github.com:them/nope.git"
    And the clone finishes with exit status "128"
    Then the story view state is "clone-failed"
    And the modal is "none"

  # Picking a branch of the Guest repo does what picking a local branch does,
  # aimed at the repository in the Sidecar: it is checked out there, and the
  # range is what that branch introduced over the *Guest repo's* own default
  # branch. The workspace Varde was started in is not a repository at all in
  # the general case, so a range resolved against it would be no range.

  Scenario: Picking a Guest repo's branch authors a story for what that branch introduced
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And "origin/HEAD" resolves to "main"
    And "main...HEAD" resolves to base "aaaaaaaaaaaa" and head "bbbbbbbbbbbb"
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I pick the branch "feature"
    Then the branch "feature" was checked out in the Guest repo
    And the story range is "main...HEAD"
    And the modal is "confirm-story"

  # The authoring session is the one the reviewer already had running: its
  # working directory is nothing Varde moves, so every path the prompt hands
  # over is absolute and points into the Sidecar.

  Scenario: The authoring prompt and its hand-over are absolute paths in the Sidecar
    Given Varde started with no folder in "/home/me/projects/theirs"
    And an AI session is running in the AI pane
    And git is installed
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And "origin/HEAD" resolves to "main"
    And "main...HEAD" resolves to base "aaaaaaaaaaaa" and head "bbbbbbbbbbbb"
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I pick the branch "feature"
    And I confirm the story range
    Then the authoring prompt names the story set in the Sidecar
    And the story context file was written into the Sidecar

  Scenario: Authoring a Guest repo's story set neither stops nor moves the AI session
    Given Varde started with no folder in "/home/me/projects/theirs"
    And an AI session is running in the AI pane
    And git is installed
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And "origin/HEAD" resolves to "main"
    And "main...HEAD" resolves to base "aaaaaaaaaaaa" and head "bbbbbbbbbbbb"
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I pick the branch "feature"
    And I confirm the story range
    Then no new AI session was started
    And no AI session was stopped
    And the terminal has run nothing but the clone

  # Walking a Guest repo's Story set is the local walk with the Guest repo's
  # own files under it: the Site's text is read from the clone, and the Step
  # opens the file inside it.

  Scenario: A Guest repo's story step is read and opened inside the clone
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And "origin/HEAD" resolves to "main"
    And "main...HEAD" resolves to base "aaaaaaaaaaaa" and head "bbbbbbbbbbbb"
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And the Guest repo's file "src/theirs.rs" holds:
      """
      fn theirs() {}
      """
    And I pick the branch "feature"
    And I confirm the story range
    And the story artifact arrives:
      """
      {
        "protocolVersion": 2,
        "title": "Theirs",
        "range": { "spelling": "main...HEAD", "base": "aaaaaaaaaaaa", "head": "bbbbbbbbbbbb" },
        "stories": [{
          "id": "s1",
          "name": "What they did",
          "premise": "one function",
          "steps": [{
            "id": "s1e1",
            "name": "the function",
            "claim": "it is added",
            "why": "it is what the branch adds",
            "site": {
              "file": "src/theirs.rs", "side": "new", "kind": "changed",
              "from": 1, "to": 1
            }
          }]
        }]
      }
      """
    Then the site text of step 1 is "fn theirs() {}"
    When I enter the story "What they did"
    Then the Guest repo's file "src/theirs.rs" was opened in the editor
    And the site mark is drawn on the file on screen
    And step 1 is not stale

  Scenario: A Guest repo's step shows its range's diff on d
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And "origin/HEAD" resolves to "main"
    And "main...HEAD" resolves to base "aaaaaaaaaaaa" and head "bbbbbbbbbbbb"
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And the Guest repo's file "src/theirs.rs" held:
      """
      fn mine() {}
      """
    And the Guest repo's file "src/theirs.rs" holds:
      """
      fn theirs() {}
      """
    And I pick the branch "feature"
    And I confirm the story range
    And the story artifact arrives:
      """
      {
        "protocolVersion": 2,
        "title": "Theirs",
        "range": { "spelling": "main...HEAD", "base": "aaaaaaaaaaaa", "head": "bbbbbbbbbbbb" },
        "stories": [{
          "id": "s1",
          "name": "What they did",
          "premise": "one function",
          "steps": [{
            "id": "s1e1",
            "name": "the function",
            "claim": "it is renamed",
            "why": "it is what the branch changes",
            "site": {
              "file": "src/theirs.rs", "side": "new", "kind": "changed",
              "from": 1, "to": 1
            }
          }]
        }]
      }
      """
    And I enter the story "What they did"
    And I press "d"
    Then line 1 of "src/theirs.rs" is marked as added
    And the code shows the removed rows:
      | under | text         |
      | 0     | fn mine() {} |

  # Staleness, unchanged: a Step is judged against what its Site's lines hold
  # on disk, and for a Guest repo that disk is the clone. This is why the
  # branch is checked out there in the first place.

  Scenario: A step whose site in the clone no longer holds its text says so
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And "origin/HEAD" resolves to "main"
    And "main...HEAD" resolves to base "aaaaaaaaaaaa" and head "bbbbbbbbbbbb"
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And the Guest repo's file "src/theirs.rs" holds:
      """
      fn theirs() {}
      """
    And I pick the branch "feature"
    And I confirm the story range
    And the story artifact arrives:
      """
      {
        "protocolVersion": 2,
        "title": "Theirs",
        "range": { "spelling": "main...HEAD", "base": "aaaaaaaaaaaa", "head": "bbbbbbbbbbbb" },
        "stories": [{
          "id": "s1",
          "name": "What they did",
          "premise": "one function",
          "steps": [{
            "id": "s1e1",
            "name": "the function",
            "claim": "it is added",
            "why": "it is what the branch adds",
            "site": {
              "file": "src/theirs.rs", "side": "new", "kind": "changed",
              "from": 1, "to": 1
            }
          }]
        }]
      }
      """
    And I enter the story "What they did"
    And the Guest repo's file "src/theirs.rs" holds:
      """
      fn something_else() {}
      """
    Then step 1 is stale as "text-changed"

  # A file that is deleted when Varde quits is a file editing cannot help:
  # the buffer reads, and every key that would change it is refused out loud.

  Scenario: A buffer on a Guest repo's file refuses an edit
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And the Guest repo's file "src/theirs.rs" holds:
      """
      fn theirs() {}
      """
    And the Guest repo's file "src/theirs.rs" is opened in the editor
    And I press "x" in the editor
    Then the editor refuses with "guest-read-only"
    And the buffer is unchanged
    And the buffer holds:
      """
      fn theirs() {}
      """

  # A Guest repo is the repository under review for as long as it is under
  # review. `:story?` with no URL is this folder again, so the clone stops
  # being it — otherwise the next branch picked would be checked out in
  # somebody else's repository.

  Scenario: A ":story?" with no URL after a Guest repo is this folder's own again
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the project is a git repository
    And the working tree has no changes
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I press the key "Escape"
    And I run ":story?"
    And I pick the branch "main"
    Then the branch "main" was checked out

  # Reviewing two branches of one repository downloads it once. A second `:story?` on a URL this
  # session already has in its Sidecar fetches into that copy: a clone into a directory that already
  # exists only fails, and the fetch is what puts a branch pushed since the clone in the picker.

  Scenario: A second ":story?" on the same URL fetches instead of cloning again
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I press the key "Escape"
    And I run ":story? git@github.com:them/theirs.git"
    Then the terminal has fetched "git@github.com:them/theirs.git"
    And the story view state is "fetching"

  Scenario: The branch list after a fetch holds a branch pushed since the clone
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I press the key "Escape"
    And the branch "origin/hotfix" appears on the remote
    And I run ":story? git@github.com:them/theirs.git"
    And the fetch finishes with exit status "0"
    Then the picker lists:
      | hotfix |
      | main   |

  # The refusal a failed clone gets, for the same reason — and the copy already on disk stays: it is
  # still the repository under review, and a download of something newer failing is no reason to
  # throw away the one that arrived.

  Scenario: A fetch that failed is refused with its exit status, and the clone stays
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I press the key "Escape"
    And I run ":story? git@github.com:them/theirs.git"
    And the fetch finishes with exit status "128"
    Then the story view state is "fetch-failed"
    And the modal is "none"
    And the Guest repo is still there

  Scenario: A URL this session has not downloaded is still cloned
    Given Varde started with no folder in "/home/me/projects/theirs"
    And git is installed
    And the repository has branches:
      | name | kind  | seconds |
      | main | local | 200     |
    When I run ":story? git@github.com:them/theirs.git"
    And the clone finishes with exit status "0"
    And I press the key "Escape"
    And I run ":story? git@github.com:them/other.git"
    Then the terminal has cloned "git@github.com:them/other.git" into the Sidecar
    And the story view state is "cloning"
