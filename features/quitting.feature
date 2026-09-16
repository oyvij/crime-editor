Feature: Closing and quitting

  :q closes the file you have open, as it closes a window in vim. It refuses
  while *that* buffer has unsaved edits — never because some other one does,
  which is what made one unsaved file lock every clean buffer open; :q!
  discards them.

  :qa is the clear-up: it closes every buffer with nothing unsaved, keeps the
  ones that have, and says which ones it kept. It refuses nothing — skipping a
  dirty buffer is the point, not a failure — and leaves the editor on a buffer
  that still exists, or on none with the tree in focus. :qa! takes the dirty
  ones with it.

  Leaving CRIME altogether is not on the `:` line at all: Ctrl+Q and the
  palette's q are the gestures for it, wanted from anywhere and unambiguous,
  so a mistyped clear-up cannot take the session with it.

  Per-project state is written on the way out either way, so reopening puts the
  tree and the layout back.

  Background:
    Given the workspace root is "/home/me/projects/crime"

  Scenario: Closing the buffer in front of you while a different one is dirty
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    When I close the buffer
    Then the open buffers are:
      | src/one.js |
    And the current buffer is "src/one.js"
    And the editor never said "unsaved-changes"

  Scenario: Force closing discards the buffer in front of you and keeps the rest
    Given "src/one.js" is open in the editor with unsaved edits
    And "src/two.js" is open in the editor with unsaved edits
    When I force close the buffer
    Then the open buffers are:
      | src/one.js |
    And "src/one.js" has unsaved edits

  Scenario: Quitting is refused while any buffer is dirty
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    When I quit
    Then CRIME is still running
    And the reviewer is told there are unsaved changes

  Scenario: Clearing up closes the buffers with nothing unsaved
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    And I open "src/three.js"
    When I close every clean buffer
    Then the open buffers are:
      | src/one.js |
    And "src/one.js" has unsaved edits
    And CRIME is still running

  Scenario: Clearing up says what it kept and leaves the editor on it
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    When I close every clean buffer
    Then the notice is "buffers-kept"
    And the message names "src/one.js"
    And the current buffer is "src/one.js"
    And the buffer mark for "src/two.js" is "none"
    And the buffer mark for "src/one.js" is "current-dirty"

  Scenario: Clearing up with everything clean empties the editor
    Given "src/one.js" is open in the editor with no unsaved edits
    And I open "src/two.js"
    When I close every clean buffer
    Then the editor has no file open
    And the file tree pane has focus
    And the notice is "buffers-closed"
    And CRIME is still running

  Scenario: Clearing up with everything dirty keeps everything and refuses nothing
    Given "src/one.js" is open in the editor with unsaved edits
    And "src/two.js" is open in the editor with unsaved edits
    When I close every clean buffer
    Then the open buffers are:
      | src/one.js |
      | src/two.js |
    And the current buffer is "src/two.js"
    And the notice is "buffers-kept"
    And the message names "src/one.js, src/two.js"
    And the editor never said "unsaved-changes"

  Scenario: Forcing the clear-up takes the dirty buffers with it
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    When I force close every buffer
    Then the editor has no file open
    And the file tree pane has focus
    And the notice is "buffers-closed"
    And CRIME is still running

  Scenario: Closing a clean buffer empties the editor
    Given "src/tree.js" is open in the editor with no unsaved edits
    When I close the buffer
    Then the editor has no file open
    And CRIME is still running

  Scenario: Closing is refused while the buffer in front of you is dirty
    Given "src/tree.js" is open in the editor with unsaved edits
    When I close the buffer
    Then "src/tree.js" has unsaved edits
    And the reviewer is told there are unsaved changes

  Scenario: Force closing discards the edits
    Given "src/tree.js" is open in the editor with unsaved edits
    When I force close the buffer
    Then the editor has no file open
    And CRIME is still running

  Scenario: Closing the buffer never quits CRIME
    Given "src/tree.js" is open in the editor with no unsaved edits
    When I close the buffer
    Then CRIME is still running

  Scenario: Quitting a clean session
    Given "src/tree.js" is open in the editor with no unsaved edits
    When I quit
    Then CRIME exits
    And the project state was saved

  Scenario: Quitting is refused while a buffer is dirty
    Given "src/tree.js" is open in the editor with unsaved edits
    When I quit
    Then CRIME is still running
    And the reviewer is told there are unsaved changes

  Scenario: Forcing a quit abandons the edits
    Given "src/tree.js" is open in the editor with unsaved edits
    When I force quit
    Then CRIME exits
    And the project state was saved
