Feature: Folding a block away

  A block of code can be folded away from the command line. `:toggle` folds the
  block the cursor is in — or opens it again if it is already folded — and
  `:toggle!` answers for the whole file: every block at once, and open again on
  the next press.

  A block is where the lines are: a line that opens one keeps a toggle in its
  gutter, so the affordance is on the line rather than in a menu. What is
  asserted here is which lines the editor hides and which state a toggle is in,
  never the glyph drawn for it — a glyph is a theme's business.

  Background:
    Given the workspace root is "/home/me/projects/varde"

  Scenario: The block the cursor is in folds away
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let x = 1;
          let y = 2;
      }
      """
    And the cursor is on line 2
    When I run ":toggle" in the editor
    Then the editor hides lines 2 to 3
    And the fold toggle on line 1 is folded
    And the cursor is on line 1

  Scenario: Toggling the same block again opens it
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let x = 1;
      }
      """
    And the cursor is on line 2
    And I run ":toggle" in the editor
    When I run ":toggle" in the editor
    Then the editor hides no lines
    And the fold toggle on line 1 is open

  Scenario: The innermost block is the one that folds
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          if ok {
              go();
          }
      }
      """
    And the cursor is on line 3
    When I run ":toggle" in the editor
    Then the editor hides lines 3 to 3
    And the fold toggle on line 2 is folded
    And the fold toggle on line 1 is open

  Scenario: A line that opens no block folds nothing
    Given "src/main.rs" is open in the editor holding:
      """
      use std::fs;
      use std::io;
      """
    And the cursor is on line 1
    When I run ":toggle" in the editor
    Then the editor hides no lines
    And line 1 carries no fold toggle

  Scenario: Every block folds at once
    Given "src/main.rs" is open in the editor holding:
      """
      fn one() {
          go();
      }
      fn two() {
          go();
      }
      """
    When I run ":toggle!" in the editor
    Then the editor hides lines 2 to 2
    And the editor hides lines 5 to 5
    And the fold toggle on line 1 is folded
    And the fold toggle on line 4 is folded

  Scenario: Folding everything a second time opens it all again
    Given "src/main.rs" is open in the editor holding:
      """
      fn one() {
          go();
      }
      fn two() {
          go();
      }
      """
    And I run ":toggle!" in the editor
    When I run ":toggle!" in the editor
    Then the editor hides no lines
    And the fold toggle on line 1 is open

  Scenario: A click under a folded block lands on the line it points at
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          go();
      }
      let after = 1;
      """
    And the cursor is on line 1
    And I run ":toggle" in the editor
    When I click at line 3 column 1 in the editor
    Then the cursor is on line 4

  Scenario: The caret steps over a folded block rather than into it
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let x = 1;
          let y = 2;
      }
      let after = 1;
      """
    And the cursor is on line 1
    And I run ":toggle" in the editor
    When I press the Down arrow in the editor
    Then the cursor is on line 4
    When I press the Up arrow in the editor
    Then the cursor is on line 1

  Scenario: Enter on the dots opens the block and the caret takes their place
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let x = 1;
      }
      """
    And the cursor is on line 1
    And I run ":toggle" in the editor
    And I press "$" in the editor
    When I press "Enter" in the editor
    Then the editor hides no lines
    And the fold toggle on line 1 is open
    And the cursor is at line 1 column 12

  Scenario: Clicking the toggle in the gutter opens the block
    Given "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let x = 1;
      }
      """
    And the cursor is on line 1
    And I run ":toggle" in the editor
    When I click the fold toggle on row 1 in the editor
    Then the editor hides no lines
    And the fold toggle on line 1 is open

  Scenario: Folding with nothing open is refused out loud
    Given no file is open in the editor
    When I run ":toggle" in the editor
    Then the editor refuses with "no-file-open"
    And the editor hides no lines
