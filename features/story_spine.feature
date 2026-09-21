Feature: The spine shows the shape of a change before any of it is walked

  The spine is the ordered list of a change's Stories and, under each, its Steps. It is what the
  reviewer reads to decide what to walk. It lives in the tree pane, where `t` toggles it against the
  changed-files list that Review view shows — the list is never replaced, only joined.

  Under the Stories sits the Remainder: the hunks no Step claims. It is a count and a list, never a
  ratio. A percentage needs a denominator, every denominator is a judgement about which files deserve
  a reviewer's eye, and a low one teaches the reviewer to ignore the line. Deletions are counted
  separately, because "this change deleted nothing" and "twelve lines went and nobody walked them"
  are opposite situations that a single zero cannot tell apart.

  Hunks are computed by Varde from the two sides of the change, with Varde's own pinned diff options,
  so both sides of the subtraction agree. A Story never authors its own coverage.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And the project is a git repository
    And "src/keys.rs" held:
      """
      fn route(k: Key) -> Event {
          match k {
              Key::Char(c) => Event::Type(c),
              _ => Event::Ignored,
          }
      }
      """
    And "src/keys.rs" now holds:
      """
      fn route(k: Key) -> Event {
          match k {
              Key::Char(c) => Event::Type(c),
              other => Event::Raw(other),
          }
      }
      """
    And "src/mouse.rs" held:
      """
      fn hit(p: Point) -> Pane {
          Pane::Editor
      }
      """
    And "src/mouse.rs" now holds:
      """
      fn hit(p: Point) -> Pane {
          layout::pane_at(p)
      }
      """

  Scenario: The tree pane toggles between the changed files and the spine
    Given a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | changed | 4    | 4  |
    And I opened Story view
    When I press "t"
    Then the tree pane shows the review list
    And the review list shows:
      | src/keys.rs |
      | src/mouse.rs |
    When I press "t"
    Then the tree pane shows the spine

  Scenario: The spine lists each story with its step count
    Given a story set for this change holds:
      | story              | steps |
      | Keys reach the child | 2   |
      | Clicks find a pane   | 1   |
    When I open Story view
    Then the spine lists:
      | name                 | steps |
      | Keys reach the child | 2     |
      | Clicks find a pane   | 1     |

  Scenario: A story whose steps no longer match carries a stale count
    Given a story set for this change claims:
      | file        | side | kind    | from | to | text                             |
      | src/keys.rs | new  | changed | 4    | 4  | other => Event::Raw(other),      |
      | src/mouse.rs| new  | changed | 2    | 2  | THIS IS NOT WHAT THE FILE HOLDS  |
    When I open Story view
    Then the spine reports 1 stale step

  Scenario: The remainder counts the hunks no step claims
    Given a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | changed | 4    | 4  |
    When I open Story view
    Then the remainder holds 1 unclaimed hunk
    And the remainder lists:
      | src/mouse.rs |

  Scenario: A site claims a hunk by overlapping it, not by containing it
    Given a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | changed | 1    | 2  |
    When I open Story view
    Then the remainder holds 1 unclaimed hunk

  Scenario: A context site claims nothing
    Given a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | context | 4    | 4  |
      | src/mouse.rs| new  | changed | 2    | 2  |
    When I open Story view
    Then the remainder holds 1 unclaimed hunk
    And the remainder lists:
      | src/keys.rs |

  Scenario: Two stories claiming the same hunk have it counted once
    Given a story set for this change claims:
      | story                | file        | side | kind    | from | to |
      | Keys reach the child | src/keys.rs | new  | changed | 4    | 4  |
      | Clicks find a pane   | src/keys.rs | new  | changed | 4    | 4  |
    When I open Story view
    Then the remainder holds 1 unclaimed hunk

  Scenario: The remainder recomputes when the change grows under it
    Given a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | changed | 4    | 4  |
    And I opened Story view
    When "src/pty.rs" now holds:
      """
      fn feed(b: &[u8]) {}
      """
    Then the remainder holds 2 unclaimed hunks

  Scenario: A range that deletes something says how much of it was walked
    Given "src/dead.rs" held:
      """
      fn unused() {}
      """
    And "src/dead.rs" is gone
    And a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | changed | 4    | 4  |
    When I open Story view
    Then the remainder reports 1 unwalked deletion

  Scenario: A range that deletes nothing has no deletions line at all
    Given a story set for this change claims:
      | file        | side | kind    | from | to |
      | src/keys.rs | new  | changed | 4    | 4  |
    When I open Story view
    Then the remainder reports no deletions line

  Scenario: The spine has no filter box
    Given a story set for this change holds:
      | story                | steps |
      | Keys reach the child | 2     |
    When I open Story view
    Then the tree pane has no filter box

  Scenario: The spine shows the loaded title above the Story rows
    Given a story set for this change holds:
      | story                | steps |
      | Keys reach the child | 2     |
    When I open Story view
    Then the spine's title is "Title"

  Scenario: Nothing authored yet has no title to show
    When I open Story view
    Then the spine has no title

  Scenario: The spine's title costs the story budget one row
    Given a story set for this change holds:
      | story                | steps |
      | Keys reach the child | 2     |
    When I open Story view
    Then the spine reserves a row for the title

  Scenario: A view with no loaded title reserves no row for one
    When I open Story view
    Then the spine reserves no row for the title

  Scenario: The title's row pulls the spine's view back one row sooner
    Given the screen is 12 rows by 40 columns
    And a story set for this change holds:
      | story  | steps |
      | Story1 | 1     |
      | Story2 | 1     |
      | Story3 | 1     |
      | Story4 | 1     |
      | Story5 | 1     |
      | Story6 | 1     |
      | Story7 | 1     |
      | Story8 | 1     |
    And I opened Story view
    When I press "Down"
    And I press "Down"
    And I press "Down"
    And I press "Down"
    And I press "Down"
    Then the spine view starts at row 2
