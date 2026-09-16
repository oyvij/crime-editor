Feature: Review view shows what has changed

  Review view lists everything uncommitted: modified, staged, and untracked files alike,
  measured against HEAD. It is the view for looking at what was just written — usually by
  the AI — before any of it is committed.

  The diff shown for a file is read-only; editing it means opening the file in
  Edit view, which `features/reviewing.feature` covers.

  Git-ignored files never appear. When there is nothing to review, the view still opens and
  says why, and the two reasons are distinguishable: a folder that is not a repository is
  not the same situation as a repository with a clean tree.

  Background:
    Given the workspace root is "/home/me/projects/crime"

  Scenario: Modified, staged and untracked files all appear
    Given the project is a git repository
    And the working tree contains:
      | path           | git status |
      | src/landing.js | modified   |
      | src/tree.js    | staged     |
      | src/new.js     | untracked  |
      | README.md      | committed  |
    When I open Review view
    Then the review list shows:
      | src/landing.js |
      | src/tree.js    |
      | src/new.js     |

  Scenario: Git-ignored files are never listed
    Given the project is a git repository
    And the working tree contains:
      | path           | git status |
      | src/landing.js | modified   |
      | dist/bundle.js | ignored    |
      | .crime/state.json | ignored |
    When I open Review view
    Then the review list shows:
      | src/landing.js |

  Scenario: A folder that is not a git repository explains itself
    Given the project is not a git repository
    When I open Review view
    Then the review list is empty
    And the review view state is "not-a-git-repository"

  Scenario: A clean working tree is a different empty state
    Given the project is a git repository
    And the working tree has no changes
    When I open Review view
    Then the review list is empty
    And the review view state is "no-changes"
