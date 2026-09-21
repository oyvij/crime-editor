Feature: A picked word is echoed where it is used again

  Picking a word answers "where else is this?" without typing it into a search.
  Every other occurrence in the same file is marked, quieter than the pick
  itself — the echo is a hint about the file, not a second selection.

  Matching is exact, unlike `/`'s: a pick is the text itself rather than a query
  somebody typed, so a word differing in case is a different word, and there is
  no convenience in lowercasing what nobody spelled.

  Only a word: a pick that spans lines is a passage, and a pty or a Preview pick
  is characters read off a screen with no buffer to look through.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And "src/main.rs" is open in the editor holding:
      """
      fn update(state)
      let Update = 1
      fn other(state)
      """

  Scenario: The picked word is echoed where it appears again
    When I drag across "state" in the editor pane
    Then the echoed occurrences are:
      | 3 | 10 |

  Scenario: The pick itself is not one of its own echoes
    When I drag in the editor pane from line 3 column 10 to line 3 column 14
    Then the echoed occurrences are:
      | 1 | 11 |

  Scenario: A word used nowhere else echoes nothing
    When I drag across "let" in the editor pane
    Then nothing is echoed

  Scenario: A word differing only in case is a different word
    When I drag across "update" in the editor pane
    Then nothing is echoed

  Scenario: A pick spanning lines is a passage, not a word
    When I drag in the editor pane from line 1 column 11 to line 3 column 14
    Then nothing is echoed

  Scenario: With nothing picked nothing is echoed
    Then nothing is echoed

  Scenario: What the in-file search left as the selection is echoed too
    Given I press "/" in the editor
    And I type "state" into the in-file search
    When I press Enter during the in-file search
    Then the echoed occurrences are:
      | 3 | 10 |
