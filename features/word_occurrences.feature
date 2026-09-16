Feature: The word under the cursor is marked

  Resting the cursor in a word marks that word, and every other place the same
  word appears in the file, with a wash faint enough to read through — the way
  an editor shows you where a name is used without being asked. It is not a
  selection: nothing is picked, nothing is copied, and the marking says only
  where the word is.

  Whole words only, and case as written: `state` inside `stated` is a different
  word, and `State` is a different name.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And "src/main.rs" is open in the editor holding:
      """
      let state = 1
      let stated = 2
      print(state)
      let State = 4
      """

  Scenario: A keyword under the cursor marks every place it is used
    When the cursor is at line 1 column 1
    Then the word under the cursor is marked at:
      | line | column |
      | 1    | 1      |
      | 2    | 1      |
      | 4    | 1      |

  Scenario: A name is marked where it appears, not where it is part of a longer word
    When the cursor is at line 1 column 5
    Then the word under the cursor is marked at:
      | line | column |
      | 1    | 5      |
      | 3    | 7      |

  Scenario: The cursor between words marks nothing
    When the cursor is at line 1 column 4
    Then no word is marked
