Feature: Indent guides

  The editor draws a light guide down each level of indentation, so how far a
  block reaches is visible without counting spaces, and every space is drawn as a dot so the shape
  of a line is visible without counting them either. The guide of the block the cursor is inside is
  drawn heavier than the rest, which is the one thing about the guides the cursor decides, along with which
  bracket pair is marked.

  What is asserted here is which column holds a guide and which guide is the
  cursor's — never the glyph or the colour, which are the edge's and change
  with a theme.

  A bracket is marked only while the cursor is inside the pair it belongs to,
  and only the innermost such pair: marking every bracket on screen puts a box
  on punctuation nobody is looking at, which is noise rather than context.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And "src/main.rs" is open in the editor holding:
      """
      fn a() {
          if x {
              foo();
              bar();
          }
          baz();
      }
      """

  Scenario: Every level of indentation starts a guide
    Then the indent guides on line 3 are at columns "0, 4"

  Scenario: An unindented line has no guide
    Then line 1 has no indent guides

  Scenario: The guide of the block holding the cursor is heavy
    Given the cursor is at line 3 column 9
    Then the indent guide on line 3 at column 4 is heavy
    And the indent guide on line 3 at column 0 is light

  Scenario: The heavy guide runs the length of the block, not the cursor's line
    Given the cursor is at line 3 column 9
    Then the indent guide on line 4 at column 4 is heavy
    And no indent guide on line 6 is heavy

  Scenario: A line that opens a block is inside the block it opens
    Given the cursor is at line 2 column 5
    Then the indent guide on line 3 at column 4 is heavy

  Scenario: The guides are the same in insert mode
    Given the cursor is at line 3 column 9
    And the editor mode is insert
    Then the indent guides on line 3 are at columns "0, 4"
    And the indent guide on line 3 at column 4 is heavy

  Scenario: A blank line keeps the guide its neighbours share
    Given "src/blank.rs" is open in the editor holding:
      """
      fn a() {
          one();

          two();
      }
      """
    Then the indent guides on line 3 are at columns "0"

  Scenario: The pair the cursor is inside is marked
    When the cursor is at line 3 column 1
    Then the marked brackets are at:
      | line | column |
      | 2    | 10     |
      | 5    | 5      |

  Scenario: The innermost pair the cursor is inside wins
    Given "src/nest.rs" is open in the editor holding:
      """
      call(a(b));
      """
    When the cursor is at line 1 column 8
    Then the marked brackets are at:
      | line | column |
      | 1    | 7      |
      | 1    | 9      |

  Scenario: A cursor inside no pair marks nothing
    When the cursor is at line 1 column 1
    Then no brackets are marked

  Scenario: A bracket the cursor rests on is inside its own pair
    Given "src/nest.rs" is open in the editor holding:
      """
      call(a);
      """
    When the cursor is at line 1 column 5
    Then the marked brackets are at:
      | line | column |
      | 1    | 5      |
      | 1    | 7      |
