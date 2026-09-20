Feature: Git blame

  The editor's top border says who last committed the line the cursor is on, and the date they
  wrote it, so reading unfamiliar code answers "who do I ask about this" without leaving the file.
  Which authorship the border reports is what is asserted; the wording and where it is cut are the
  edge's, pinned in a unit test.

  It is the *committed* file's blame, so a buffer line has to be traced back through the diff before
  it names anybody: a line inserted above the cursor would otherwise hand the cursor's line the
  author of the line above it. A line the working tree has changed, and a line in a file the commit
  has no copy of, are nobody's yet. Outside a repository, and without git, the border says nothing
  at all rather than reporting an absence on every file — the Change bar's precedent.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And git is installed
    And the project is a git repository
    And the last commit holds "src/main.rs" as:
      """
      fn main() {
          run();
      }
      """
    And the last commit authored "src/main.rs" as:
      | line | author       | date       |
      | 1    | Ada Lovelace | 2026-01-05 |
      | 2    | Grace Hopper | 2026-02-11 |
      | 3    | Ada Lovelace | 2026-01-05 |
    And "src/main.rs" is open in the editor holding:
      """
      fn main() {
          run();
      }
      """

  Scenario: The cursor's line names the hand that wrote it and when
    Given the cursor is at line 1 column 1
    Then the editor pane reports the line as authored by "Ada Lovelace" on "2026-01-05"

  Scenario: Moving to a line another hand wrote changes what the border says
    Given the cursor is at line 1 column 1
    When the cursor is at line 2 column 1
    Then the editor pane reports the line as authored by "Grace Hopper" on "2026-02-11"

  Scenario: A line the working tree has changed is nobody's yet
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          run(1);
      }
      """
    And the cursor is at line 2 column 1
    Then the editor pane reports the line as "not-committed-yet"

  Scenario: A line below an insertion still names its own author
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          setup();
          run();
      }
      """
    And the cursor is at line 3 column 1
    Then the editor pane reports the line as authored by "Grace Hopper" on "2026-02-11"

  Scenario: A line the commit has no copy of the file for is nobody's yet
    Given "src/new.rs" is open in the editor holding:
      """
      fn new() {}
      """
    And the cursor is at line 1 column 1
    Then the editor pane reports the line as "not-committed-yet"

  Scenario: A folder that is not a repository has nothing to report
    Given the project is not a git repository
    And the cursor is at line 1 column 1
    Then the editor pane reports no authorship

  Scenario: A machine without git has nothing to report
    Given git is not installed
    And the cursor is at line 1 column 1
    Then the editor pane reports no authorship

  Scenario: Review view's title is not the editor's
    Given the cursor is at line 1 column 1
    When I switch to review view
    Then the editor pane reports no authorship

  Scenario: A Preview reports the source line its row came from
    Given the last commit holds "notes.md" as:
      """
      # Title

      A paragraph.
      """
    And the last commit authored "notes.md" as:
      | line | author       | date       |
      | 1    | Ada Lovelace | 2026-01-05 |
      | 2    | Ada Lovelace | 2026-01-05 |
      | 3    | Grace Hopper | 2026-02-11 |
    And "notes.md" is open in the editor holding:
      """
      # Title

      A paragraph.
      """
    And the cursor is on the row holding "A paragraph."
    Then the editor pane reports the line as authored by "Grace Hopper" on "2026-02-11"
