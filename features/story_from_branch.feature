Feature: Authoring a story set for a branch

  The review Varde is built for is the review of a branch. `:story?` lists the branches of whatever
  repository the folder is — local and remote-tracking, deduped by short name, most recent commit
  first — and picking one checks it out and authors a Story set for what that branch introduced.

  Picking a branch checks it out because a Step's staleness is judged by reading what its Site's
  lines currently hold on disk: a Story set for a branch that is not checked out would report every
  Step stale. Varde does not check the original branch back out afterwards — a second checkout can
  fail if files were touched during the review, leaving the reviewer somewhere neither they nor
  Varde chose. Story view says which branch it is on and which one was left instead.

  A dirty working tree is refused up front with an instruction to commit, because Varde never
  checks out over work nobody has saved.

  Background:
    Given the workspace root is "/home/me/projects/varde"

  Scenario: Picking a branch checks it out and authors a story for what it introduced
    Given the project is a git repository
    And the working tree has no changes
    And the repository has branches:
      | name          | kind   | seconds |
      | feature       | local  | 300     |
      | origin/feature| remote | 300     |
      | main          | local  | 200     |
    And "origin/HEAD" resolves to "main"
    When I run ":story?"
    And I pick the branch "feature"
    Then the branch "feature" was checked out
    And the story range is "main...HEAD"
    And the modal is "confirm-story"

  Scenario: The story view names the branch it is on and the one that was left
    Given the project is a git repository
    And the working tree has no changes
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    And I was on the branch "main"
    And "origin/HEAD" resolves to "main"
    When I run ":story?"
    And I pick the branch "feature"
    Then the story view is on the branch "feature" and left the branch "main"

  Scenario: Varde's own directory is not the reviewer's uncommitted work
    Given the project is a git repository
    And the working tree contains:
      | path              | git status |
      | .varde/state.json | untracked  |
      | .varde/risk.json  | untracked  |
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
    When I run ":story?"
    Then the modal is "branches"

  Scenario: The picker is refused in a folder that is not a repository
    Given the project is not a git repository
    When I run ":story?"
    Then the story view state is "not-a-git-repository"
    And the modal is "none"
    And no branch was checked out
    And no command has been executed
    And no keys were sent to the AI pane

  Scenario: The picker is refused on a dirty working tree, told to commit first
    Given the project is a git repository
    And the working tree contains:
      | path        | git status |
      | src/keys.rs | modified   |
    When I run ":story?"
    Then the story view state is "working-tree-dirty"
    And the modal is "none"
    And no branch was checked out
    And no command has been executed
    And no keys were sent to the AI pane

  Scenario: Typing narrows the list, and clearing what was typed brings it back
    Given the project is a git repository
    And the working tree has no changes
    And the repository has branches:
      | name         | kind  | seconds |
      | fix-the-tree | local | 300     |
      | feature      | local | 200     |
      | main         | local | 100     |
    When I run ":story?"
    And I type "fea" in the picker
    Then the picker lists:
      | feature |
    When I press the key "Backspace"
    And I press the key "Backspace"
    And I press the key "Backspace"
    Then the picker lists:
      | fix-the-tree |
      | feature      |
      | main         |

  Scenario: A filter matching nothing leaves nothing to pick
    Given the project is a git repository
    And the working tree has no changes
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
    When I run ":story?"
    And I type "zzz" in the picker
    Then the picker lists nothing
    When I press the key "Enter"
    Then no branch was checked out
    And the modal is "branches"

  Scenario: The selection stays on a row that is still shown
    Given the project is a git repository
    And the working tree has no changes
    And the repository has branches:
      | name    | kind  | seconds |
      | feature | local | 300     |
      | main    | local | 200     |
      | mainly  | local | 100     |
    When I run ":story?"
    And I press the key "Down"
    And I press the key "Down"
    And I type "main" in the picker
    And I press the key "Enter"
    Then the branch "mainly" was checked out
