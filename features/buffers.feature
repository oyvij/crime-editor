Feature: Several files open at once

  CRIME keeps every file you open, without a tab bar. What is open shows as a
  mark in the file tree — the list of files you already have — and as a strip of
  dots on the editor's bottom edge, one per buffer, filled for the one you are
  in. gt and gT step through them; the dots are clickable.

  Browsing does not fill that strip: moving the tree selection opens a file as a
  preview, which the next preview replaces. Opening it properly — Enter, or a
  click — makes it stay.

  Switching buffers selects the same file in the tree, so the two views never
  disagree about where you are — opening the folders leading to it, since a file
  in a folder nobody has walked into is not a row yet.

  Background:
    Given the workspace root is "/home/me/projects/crime"

  Scenario: Opening a second file keeps the first
    Given "src/one.js" is open in the editor
    When I open "src/two.js"
    Then the open buffers are:
      | src/one.js |
      | src/two.js |
    And the current buffer is "src/two.js"

  Scenario: Unsaved edits survive opening another file
    Given "src/one.js" is open in the editor with unsaved edits
    When I open "src/two.js"
    Then "src/one.js" has unsaved edits

  Scenario: gt and gT step buffers without a modifier key
    Given "src/one.js" is open in the editor
    And I open "src/three.js"
    And I open "src/two.js"
    When I press "gt" in the editor
    Then the current buffer is "src/one.js"

  Scenario: gT steps the other way
    Given "src/one.js" is open in the editor
    And I open "src/three.js"
    And I open "src/two.js"
    When I press "gT" in the editor
    Then the current buffer is "src/three.js"

  Scenario: Switching selects the same file in the tree
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    When I press "gT" in the editor
    Then the tree selection is "src/one.js"

  Scenario: Clicking a dot switches to that buffer
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    When I click buffer dot 1
    Then the current buffer is "src/one.js"

  Scenario: Previewing replaces the previous preview
    Given the folder "src" is expanded
    And "src/one.js" is previewed
    When "src/two.js" is previewed
    Then the open buffers are:
      | src/two.js |

  Scenario: Opening a previewed file makes it stay
    Given "src/one.js" is previewed
    And I open "src/one.js"
    When "src/two.js" is previewed
    Then the open buffers are:
      | src/one.js |
      | src/two.js |

  Scenario: Selecting a file that is already open switches to it
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    When "src/one.js" is previewed
    Then the current buffer is "src/one.js"
    And "src/one.js" has unsaved edits
    And the open buffers are:
      | src/one.js |
      | src/two.js |

  Scenario: An unsaved buffer is marked differently from a saved one
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    Then the buffer mark for "src/one.js" is "dirty"
    And the buffer mark for "src/two.js" is "current"

  Scenario: The current buffer shows as unsaved once it is edited
    Given "src/one.js" is open in the editor with unsaved edits
    Then the buffer mark for "src/one.js" is "current-dirty"

  Scenario: A file with no buffer is unmarked
    Given "src/one.js" is open in the editor
    Then the buffer mark for "src/two.js" is "none"

  Scenario: Closing a buffer moves to another one
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    When I close the buffer
    Then the current buffer is "src/one.js"
    And CRIME is still running

  Scenario: Closing the last buffer empties the editor
    Given "src/one.js" is open in the editor
    When I close the buffer
    Then the editor has no file open

  # One set of open buffers, but each view shows its own: a file opened while
  # walking a story is not what the editor should hold when you go back to Edit.
  Scenario: Leaving a view does not carry its buffer into the next
    Given "src/one.js" is open in the editor
    And I open Story view
    And I open "src/two.js"
    When I open Edit view
    Then the current buffer is "src/one.js"

  Scenario: Returning to a view shows the buffer it had open
    Given "src/one.js" is open in the editor
    And I open Story view
    And I open "src/two.js"
    And I open Edit view
    And I open "src/three.js"
    When I open Story view
    Then the current buffer is "src/two.js"

  Scenario: A view that has shown nothing yet shows no buffer
    Given "src/one.js" is open in the editor
    When I open Story view
    Then the editor shows no buffer
    And the open buffers are:
      | src/one.js |

  Scenario: A view does not return to a buffer closed elsewhere
    Given "src/one.js" is open in the editor
    And I open Story view
    And I open "src/two.js"
    And I click buffer dot 1
    And I close the buffer
    When I open Edit view
    Then the editor shows no buffer

  # The files you had open are per-user state, so they live in state.json beside
  # the folders the tree had open — reopening a project puts you back in the
  # file you left, not in an empty editor.
  Scenario: Reopening a project returns to the files that were open
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And I quit
    When CRIME starts in the project
    Then the open buffers are:
      | src/one.js |
      | src/two.js |
    And the current buffer is "src/two.js"

  # Browsing is not opening, here as well as in the strip of dots: a preview is
  # the one slot the next preview takes, and remembering it would fill next
  # session's editor with whatever the tree selection last passed over.
  Scenario: A previewed file is not remembered for next time
    Given "src/one.js" is open in the editor
    And "src/two.js" is previewed
    And I quit
    When CRIME starts in the project
    Then the open buffers are:
      | src/one.js |
    And the current buffer is "src/one.js"

  Scenario: Jumping to a file in an unopened folder opens the way to it
    Given "src/deep/tree.js" is on disk under collapsed folders
    When I open "src/deep/tree.js"
    Then the tree selection is "src/deep/tree.js"
    And the file tree shows "src/deep/tree.js"

  Scenario: Stepping to a buffer in an unopened folder opens the way to it too
    Given "src/deep/tree.js" is on disk under collapsed folders
    And I open "src/deep/tree.js"
    And I open "top.js"
    And I collapse the tree
    When I press "gt" in the editor
    Then the tree selection is "src/deep/tree.js"
    And the file tree shows "src/deep/tree.js"
