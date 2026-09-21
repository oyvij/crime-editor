Feature: Change marks

  A line the buffer holds that the last commit does not carries a bar in the gutter, so every place
  a file has been changed is visible while editing it — VS Code's gutter indicator, asked for by
  screenshot. Which lines are marked is what is asserted; the glyph and the colour are the edge's.

  The mark answers the buffer as it is, saved or not: a line typed a second ago is a change. A file
  the last commit does not hold — untracked, or not in a repository at all — has nothing to be
  compared against, and carries no mark rather than a bar down every line.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And the last commit holds "src/main.rs" as:
      """
      fn main() {
          run();
      }
      """

  Scenario: The file as the commit holds it carries no mark
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          run();
      }
      """
    Then no line is marked as changed

  Scenario: A line the commit does not hold is marked
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          setup();
          run();
      }
      """
    Then lines "2" are marked as changed

  Scenario: An edited line is marked and the lines around it are not
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          run(1);
      }
      """
    Then lines "2" are marked as changed

  Scenario: Every changed place in the file is marked, not only the first
    Given "src/main.rs" is open in the editor holding:
      """
      use std::env;

      fn main() {
          log();
          run();
      }
      """
    Then lines "1, 2, 4" are marked as changed

  Scenario: An edit is a change the moment it is typed, saved or not
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          run();
      }
      """
    And the editor pane has focus
    And the cursor is at line 2 column 1
    And the editor mode is insert
    When I type "x" in the editor
    Then lines "2" are marked as changed

  Scenario: A file the commit does not hold carries no mark
    Given "src/new.rs" is open in the editor holding:
      """
      fn new() {}
      """
    Then no line is marked as changed
