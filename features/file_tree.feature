Feature: The file tree shows the workspace

  The tree is an honest view of the folder: everything is there, including dotfiles and
  .git, with nothing filtered out. Directories come first so the shape of the project stays
  stable as files come and go.

  Git-ignored paths are the one thing treated differently, and only visually — they are
  shown, but dimmed, so build output and dependencies don't read as source.

  A folder opened once stays open, so a tree explored all day only ever grows. Collapsing
  is the one gesture that shrinks it: every open folder closes at once, leaving the tree
  showing the root's own entries — the shape a project opened for the first time has, and
  a folder opens again from there the same way. It runs no command, since it changes
  nothing on disk.

  The selected row is left exactly where it is, and collapsing is given no rule of its own
  for it. A row still on screen stays selected. One inside a folder that just closed keeps
  naming a path the tree no longer draws, and the next Up or Down lands on a row that is
  there — the answer every list in Varde already gives for a selection it cannot find.

  Background:
    Given the workspace root is "/home/me/projects/varde"

  Scenario: Directories are listed before files, each alphabetically
    Given the workspace folder contains:
      | name         | kind      |
      | package.json | file      |
      | src          | directory |
      | AGENTS.md    | file      |
      | features     | directory |
    When the file tree is rendered
    Then the file tree lists, in order:
      | features     |
      | src          |
      | AGENTS.md    |
      | package.json |

  Scenario: Dotfiles are shown
    Given the workspace folder contains:
      | name       | kind      |
      | .gitignore | file      |
      | .varde     | directory |
      | src        | directory |
    When the file tree is rendered
    Then the file tree lists, in order:
      | .varde     |
      | src        |
      | .gitignore |

  Scenario: The .git folder is shown like any other dotfile
    Given the workspace folder contains:
      | name | kind      |
      | .git | directory |
      | src  | directory |
    When the file tree is rendered
    Then the file tree lists, in order:
      | .git |
      | src  |

  Scenario: A project opened for the first time shows only the top level
    Given the project has no recorded tree state
    And the workspace folder contains:
      | name | kind      |
      | src  | directory |
    And "src" contains "tree.js"
    When the file tree is rendered
    Then the file tree lists, in order:
      | src |
    And "src" is collapsed

  Scenario: Expanding a folder lists its contents
    Given the file tree shows the collapsed folder "src"
    And "src" contains "tree.js"
    When I expand "src"
    Then "src" is expanded
    And the file tree shows "src/tree.js"

  Scenario: Reopening a project restores the folders that were expanded
    Given the project state records "src" as expanded
    And "src" contains "tree.js"
    When the file tree is rendered
    Then "src" is expanded
    And the file tree shows "src/tree.js"

  Scenario: Git-ignored paths are shown but dimmed
    Given the workspace folder contains:
      | name         | kind      |
      | src          | directory |
      | node_modules | directory |
      | package.json | file      |
    And git ignores "node_modules"
    When the file tree is rendered
    Then the file tree lists, in order:
      | node_modules |
      | src          |
      | package.json |
    And "node_modules" is dimmed
    And "src" is not dimmed

  Scenario: Collapsing closes every open folder at once
    Given "src" contains "tree.js"
    And "features" contains "tree.feature"
    And the folder "src" is expanded
    And the folder "features" is expanded
    When I collapse the tree
    Then "src" is collapsed
    And "features" is collapsed
    And the file tree lists, in order:
      | features |
      | src      |
    And the file tree does not show "src/tree.js"
    And no command has been executed

  Scenario: Collapsing with nothing open changes nothing
    Given the workspace folder contains:
      | name | kind      |
      | src  | directory |
    When I collapse the tree
    Then the file tree lists, in order:
      | src |
    And no command has been executed

  Scenario: A selected row that is still there stays selected
    Given the folder "src" is expanded
    And the tree selection is "src"
    When I collapse the tree
    Then the tree selection is "src"

  Scenario: A selection inside a folder that closed lands on a row that is there
    Given "src" contains "tree.js"
    And the folder "src" is expanded
    And the tree selection is "src/tree.js"
    And I collapse the tree
    When I press "Down"
    Then the tree selection is "src"

  Scenario: Collapsing leaves the filter narrowing the tree
    Given the project contains:
      | src/tree.js  |
      | README.md    |
    And the folder "src" is expanded
    And I filter by "tree"
    When I collapse the tree
    Then the filtered files are:
      | src/tree.js |
    And the row "src" is expanded

  Scenario: Collapsing is reachable from the palette
    Given the folder "src" is expanded
    When I click the palette entry "Collapse"
    Then "src" is collapsed
