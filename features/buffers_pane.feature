Feature: The Buffers pane

  A pane listing every buffer that is open, one row per file, so what the dot strip on the editor's
  bottom edge says in three pixels is also readable as paths. It is toggled from the palette and it
  sits where the Risk list sits — beneath the tree, at the tree's width, taking those columns from
  the shell pane.

  The corner beneath the tree is one slot with several occupants, not one pane per flag: asking for
  this pane while the Risk list is showing replaces it, and neither can hide the other by accident
  because "both at once" is not a state the slot can hold.

  A row carries the same mark the editor's dots carry — filled for the buffer you are in, filled and
  coloured for one with unsaved work, hollow for one merely open — so the two views of the same fact
  cannot disagree. The pane has no actions: `Enter` or a click shows that buffer, which is a switch
  rather than a read, so nothing is opened from disk and nothing is written. `Enter` follows into the
  editor because it is the deliberate "take me there"; a click leaves the keyboard in the pane it
  clicked. The selection is a Row selection, so it is never copied as characters — a drag over the
  pane picks nothing at all, rather than characters off whatever pane lies behind it.

  Switching buffers some other way — the dots, `gt`, a hit in the tree — moves the row selection with
  it: a highlight that disagreed with the dot strip would read as a bug. From there the arrows move
  it freely.

  Background:
    Given the workspace root is "/home/me/projects/crime"

  Scenario: The palette offers the pane in the group the panes live in
    Given the view palette is shown
    Then the view palette offers "Buffers" in the group "Panes" under the key "g"

  Scenario: Picking the entry shows the pane and puts focus in it
    Given the Buffers pane is hidden
    And the view palette is shown
    When I click the palette entry "Buffers"
    Then the Buffers pane is shown
    And the buffers pane has focus

  Scenario: Picking it again hides the pane and returns focus to the tree
    Given the Buffers pane is shown
    And the view palette is shown
    When I click the palette entry "Buffers"
    Then the Buffers pane is hidden
    And the file tree pane has focus

  Scenario: Opening it while the Risk list is showing replaces it, because they are one slot
    Given the Risk list is shown
    And the view palette is shown
    When I click the palette entry "Buffers"
    Then the Buffers pane is shown
    And the Risk list is hidden

  Scenario: Opening the Risk list while the Buffers pane is showing replaces it too
    Given the Buffers pane is shown
    And the view palette is shown
    When I click the palette entry "Risk"
    Then the Risk list is shown
    And the Buffers pane is hidden

  Scenario: With the pane shown the downward gesture from the tree reaches it
    Given the Buffers pane is shown
    And the file tree pane has focus
    When I press "Alt+j"
    Then the buffers pane has focus

  Scenario: The shell pane is still reachable below the Buffers pane
    Given the Buffers pane is shown
    And the buffers pane has focus
    When I press "Alt+j"
    Then the terminal pane has focus

  Scenario: The pane is reachable back from the shell, whose left edge is where it ends
    Given the Buffers pane is shown
    And the terminal pane has focus
    When I press "Alt+h"
    Then the buffers pane has focus

  Scenario: The pane lists every open buffer by path
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    Then the Buffers pane lists:
      | src/one.js |
      | src/two.js |

  Scenario: A buffer with unsaved edits carries the dirty circle the editor draws
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    And the Buffers pane is shown
    Then the Buffers pane marks "src/one.js" as "dirty"

  Scenario: The buffer you are in is marked as current, not as merely open
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    Then the Buffers pane marks "src/two.js" as "current"
    And the Buffers pane marks "src/one.js" as "open"

  Scenario: A previewed file is in the list, and the next preview replaces it
    Given "src/one.js" is previewed
    And the Buffers pane is shown
    When "src/two.js" is previewed
    Then the Buffers pane lists:
      | src/two.js |

  Scenario: The selection moves with the arrows and does not open anything
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    When I press "Up"
    Then the Buffers pane selection is "src/one.js"
    And the current buffer is "src/two.js"
    And no file was opened in the editor

  Scenario: The selection stops at the last row, because the pane has no actions below it
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    When I press "j"
    Then the Buffers pane selection is "src/two.js"

  # The tree holds a row with actions of its own, and that row is selected: the
  # sideways gesture in this pane must reach nothing rather than reach into the
  # list the pane above it offers, which is the row under a selection nobody can
  # see from here.
  Scenario: The pane has no actions, so the sideways gesture arms nothing
    Given the file tree shows the collapsed folder "src"
    And the tree selection is "src"
    And "src/one.js" is open in the editor
    And the Buffers pane is shown
    When I press "Right"
    Then no action is selected

  Scenario: Enter opens the selected buffer and follows into the editor
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    And I press "k"
    When I press "Enter"
    Then the current buffer is "src/one.js"
    And the editor pane has focus
    And no file was opened in the editor

  Scenario: Enter on an unsaved buffer keeps its unsaved edits
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    And the Buffers pane is shown
    And I press "k"
    When I press "Enter"
    Then "src/one.js" has unsaved edits
    And no file was written

  Scenario: A click on a row opens what Enter opens and leaves the keyboard in the pane
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    When I click Buffers pane row 1
    Then the current buffer is "src/one.js"
    And the buffers pane has focus

  Scenario: Closing a buffer takes its row out of the list
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    When I close the buffer
    Then the Buffers pane lists:
      | src/one.js |

  Scenario: Clearing every clean buffer leaves the pane listing what it kept
    Given "src/one.js" is open in the editor with unsaved edits
    And I open "src/two.js"
    And the Buffers pane is shown
    When I close every clean buffer
    Then the Buffers pane lists:
      | src/one.js |

  Scenario: Clearing clean buffers pulls the selection to the buffer that is still current
    Given "src/a.js" is open in the editor
    And "src/b.js" is open in the editor with unsaved edits
    And "src/c.js" is open in the editor with unsaved edits
    And I click buffer dot 2
    And the Buffers pane is shown
    When I close every clean buffer
    Then the Buffers pane selection is "src/b.js"

  Scenario: The row selection follows the buffer you switch to some other way
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    When I click buffer dot 1
    Then the Buffers pane selection is "src/one.js"

  Scenario: A Row selection is not text, so it is never copied
    Given "src/one.js" is open in the editor
    And I open "src/two.js"
    And the Buffers pane is shown
    And the terminal shows:
      """
      $ cargo test
      test result: ok. 1003 passed
      """
    When I drag from the corner pane's first row to its second
    And I press "Ctrl+c"
    Then the clipboard holds nothing
    And the selection holds nothing

  Scenario: The wheel scrolls without moving the selection
    Given 40 files are open in the editor
    And I click buffer dot 1
    And the Buffers pane is shown
    And the Buffers pane selection is the first row
    When I scroll down 3 times with the pointer over the buffers pane
    Then the Buffers pane first visible row is row 3
    And the Buffers pane selection is the first row

  Scenario: Every event but the wheel pulls the selection back into view
    Given 40 files are open in the editor
    And I click buffer dot 1
    And the Buffers pane is shown
    And the Buffers pane selection is the first row
    And I scroll down with the pointer over the buffers pane
    When I press "j"
    Then the Buffers pane selection is in view

  Scenario: The pane's visibility is mine, and survives a restart
    Given the project ".crime/state.json" records the corner pane as "Buffers"
    When CRIME starts in the project
    Then the Buffers pane is shown

  Scenario: A corner nobody opened stays closed on a restart
    Given the project ".crime/state.json" records the corner pane as "Hidden"
    When CRIME starts in the project
    Then the corner is empty

  Scenario: A session saved by an older CRIME with the Risk list showing still opens showing it
    Given the project ".crime/state.json" records the Risk list as shown
    When CRIME starts in the project
    Then the Risk list is shown
    And the Buffers pane is hidden
