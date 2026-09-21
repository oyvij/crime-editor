Feature: Cursor history

  Where the cursor has been, and the way back. Reading code is following it: a definition three files
  away, a call site, a search hit — and then the way back to what you were actually working on. Varde
  had every way of getting somewhere and no way of returning.

  A Visit is one place the cursor has been: a file, a line, a column and the text that line held.
  Only a jump records one — a long-distance move, made deliberately: opening a file, switching
  buffers, an in-file search landing, `gg` and `G`. The **place being left** is what is recorded, so
  going back returns you where you were, which is vim's rule. Arrows and `j`/`k` are Motions, not
  jumps, and a click in the editor is not one either: a list that recorded those would be a keystroke
  log, and it would bury the four places worth returning to under four hundred.

  The list has a cursor of its own. Going back moves it towards the oldest Visit and forward towards
  the newest, and the place you were standing on when you first went back is itself recorded, so
  forward can return to it. Both ends refuse out loud: a gesture that silently does nothing is
  indistinguishable from a broken key. `Ctrl+p` and `Ctrl+n` are the modifier bindings and `gp`/`gn`
  the modifier-free route — a jump moves the cursor, so it is a Motion, and every Motion is reachable
  without a modifier. That holds in Edit view, a markdown Preview included. It does not hold in
  Review view, whose diff has no buffer to keep the half-typed `g` in, nor mid-walk through a Story,
  which has already spent `g` on jumping to a citation and `p` on stepping — so on those two surfaces
  the jump is the modifier's alone, and the cheatsheet claims only that rather than printing a key
  neither can answer. `Ctrl+Option` with the left and right arrows is a third spelling of the same
  two, on every surface the modifier bindings reach, for the hand already on the arrows. Ctrl and
  not Cmd: macOS keeps Command for itself, so no terminal reports it on an arrow at all. None of
  them is claimed from a hosted pane: Ctrl+p and Ctrl+n are readline's own history keys.

  The list is also a pane, in the Corner beneath the tree where the Risk list and the Buffers pane
  sit — one slot naming its occupant, so asking for this one while another shows is a replacement. A
  row names the file, what the cursor was standing on, and the line number; the excerpt is the word
  the cursor was on and a few more, then an ellipsis, coloured exactly as the editor colours it. One
  row action, on the row the keyboard is on: go to that file and line. A row whose line no longer
  holds what was recorded still shows what it recorded, and is not claimed to be current.

  The excerpt is the buffer's own text and never a Language server's answer. "The definition the
  cursor was standing on" is satisfied by the line the cursor was on, and making a list of where you
  have been wait on a conversation that may never answer would be a pane that is blank exactly when
  a server is missing.

  The Visits are this session's and are not written down: they name cursor positions in files that
  move between sessions. The Corner's occupant does persist, as it does for every occupant.

  Background:
    Given the workspace root is "/w"
    And the project holds:
      | file          | contents                                                |
      | src/filter.rs | pub fn matching(rows: &[Row]) -> Vec<Row> {\n    rows\n} |
    And "src/editor.rs" is open in the editor holding:
      """
      use std::fmt;

      pub struct Buffer {
          line: usize,
      }
      """

  Scenario: A jump records the place it left, so going back returns there
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "Ctrl+p"
    Then the current buffer is "src/editor.rs"
    And the cursor is at line 3 column 5

  Scenario: A motion is not a jump
    Given the cursor is at line 3 column 2
    When I press the Down arrow in the editor
    And I press "j" in the editor
    And I press "k" in the editor
    Then the cursor history is empty

  Scenario: A click in the editor is not a jump
    When I click at line 3 column 2 in the editor
    Then the cursor history is empty

  Scenario: Browsing the tree is not a jump, so the way back is the file you were reading
    Given the cursor is at line 3 column 5
    When "src/filter.rs" is previewed
    And "src/one.rs" is previewed
    And "src/two.rs" is previewed
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 3    |
    When I press the key "Ctrl+p"
    Then the current buffer is "src/editor.rs"
    And the cursor is at line 3 column 5

  Scenario: Going back and then forward returns to where going back started
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "Ctrl+p"
    And I press the key "Ctrl+n"
    Then the current buffer is "src/filter.rs"
    And the cursor is at line 2 column 1

  Scenario: Walking the history does not record the walk
    Given the cursor is at line 3 column 5
    And I jump to "src/filter.rs" line 2
    When I press the key "Ctrl+p"
    And I press the key "Ctrl+n"
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 3    |
      | src/filter.rs | 2    |
    And the Cursor history selection is row 2

  Scenario: Going back past the oldest place refuses out loud
    When I press the key "Ctrl+p"
    Then the notice is "no-earlier-place"

  Scenario: Going forward past the newest place refuses out loud
    When I press the key "Ctrl+n"
    Then the notice is "no-later-place"

  Scenario: Accepting a find is a jump, so going back returns from it
    Given the cursor is at line 1 column 1
    And I press "/" in the editor
    And I type "line" into the in-file search
    When I press Enter during the in-file search
    Then the cursor is at line 4 column 5
    And the cursor history holds:
      | file          | line |
      | src/editor.rs | 1    |
    When I press the key "Ctrl+p"
    Then the cursor is at line 1 column 1

  Scenario: The history jumps between files, not only within one
    Given the cursor is at line 4 column 1
    When I jump to "src/filter.rs" line 1
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 4    |

  Scenario: A jump within one file records the place it left too
    Given the cursor is at line 3 column 5
    When I jump to "src/editor.rs" line 5
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 3    |

  Scenario: Going back from a jump within one file returns to where it started
    Given the cursor is at line 3 column 5
    When I jump to "src/editor.rs" line 5
    And I press the key "Ctrl+p"
    Then the current buffer is "src/editor.rs"
    And the cursor is at line 3 column 5

  Scenario: A jump within one file is not stepped over on the way back
    Given the cursor is at line 3 column 5
    And I jump to "src/filter.rs" line 2
    When I jump to "src/filter.rs" line 3
    And I press the key "Ctrl+p"
    Then the current buffer is "src/filter.rs"
    And the cursor is at line 2 column 1

  Scenario: A definition in the file already open is a jump, so going back returns from it
    Given a language server for "rust" is ready
    And the cursor is at line 4 column 5
    And I press "gd" in the editor
    When the language server for "rust" answers the definition with:
      | path          | line | column |
      | src/editor.rs | 1    | 1      |
    Then the cursor is at line 1 column 1
    And no file was opened in the editor
    When I press the key "Ctrl+p"
    Then the cursor is at line 4 column 5

  Scenario: A definition the cursor already stands on records nothing
    Given a language server for "rust" is ready
    And the cursor is at line 3 column 1
    And I press "gd" in the editor
    When the language server for "rust" answers the definition with:
      | path          | line | column |
      | src/editor.rs | 3    | 1      |
    Then the cursor history is empty

  Scenario: The same place twice in a row is recorded once
    Given the cursor is at line 2 column 1
    When I jump to "src/filter.rs" line 1
    And I jump to "src/filter.rs" line 1
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 2    |

  Scenario: A place the list already ends with is not recorded again
    Given the cursor is at line 3 column 1
    And I jump to "src/editor.rs" line 5
    And I press "k" in the editor
    And I press "k" in the editor
    When I jump to "src/filter.rs" line 1
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 3    |

  Scenario: A place already recorded moves to the end rather than being recorded a second time
    Given the cursor is at line 1 column 1
    And I jump to "src/editor.rs" line 4
    And I jump to "src/editor.rs" line 1
    When I jump to "src/editor.rs" line 4
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 4    |
      | src/editor.rs | 1    |

  Scenario: Going back to a place the list already ends with steps past it, not onto a second copy
    Given the cursor is at line 1 column 1
    And I jump to "src/editor.rs" line 3
    And I jump to "src/editor.rs" line 5
    And I press "k" in the editor
    And I press "k" in the editor
    When I press the key "Ctrl+p"
    Then the cursor is at line 1 column 1
    And the cursor history holds:
      | file          | line |
      | src/editor.rs | 1    |
      | src/editor.rs | 3    |
    And the Cursor history selection is row 1

  Scenario: Going back when the only place recorded is where you stand refuses out loud
    Given the cursor is at line 3 column 1
    And I jump to "src/editor.rs" line 5
    And I press "k" in the editor
    And I press "k" in the editor
    When I press the key "Ctrl+p"
    Then the notice is "no-earlier-place"
    And the cursor is at line 3 column 1
    And the cursor history holds:
      | file          | line |
      | src/editor.rs | 3    |

  Scenario: Going back with every buffer closed still reaches the newest place
    Given the cursor is at line 3 column 5
    And I jump to "src/filter.rs" line 2
    When I close every clean buffer
    And I press the key "Ctrl+p"
    Then "src/editor.rs" was opened in the editor
    And the Cursor history selection is row 1

  Scenario: The oldest place is dropped once the history is full
    Given the cursor history is full
    When I jump to "src/filter.rs" line 1
    Then the cursor history is full
    And the cursor history holds no place in "src/oldest.rs"

  Scenario: With the history full, going back still reaches the newest place recorded
    Given the cursor history is full
    And the cursor is at line 3 column 5
    When I press the key "Ctrl+p"
    Then "src/f99.rs" was opened in the editor
    And the cursor history is full
    And the Cursor history selection is row 99

  Scenario: The modifier-free route goes back, with no modifier at all
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "g"
    And I press the key "p"
    Then the current buffer is "src/editor.rs"
    And the cursor is at line 3 column 5

  Scenario: The modifier-free route goes forward too
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "Ctrl+p"
    And I press the key "g"
    And I press the key "n"
    Then the current buffer is "src/filter.rs"
    And the cursor is at line 2 column 1

  Scenario: Ctrl+Option and the left arrow goes back
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "Ctrl+Alt+Left"
    Then the current buffer is "src/editor.rs"
    And the cursor is at line 3 column 5

  Scenario: Ctrl+Option and the right arrow goes forward
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "Ctrl+Alt+Left"
    And I press the key "Ctrl+Alt+Right"
    Then the current buffer is "src/filter.rs"
    And the cursor is at line 2 column 1

  Scenario: Option alone on an arrow is still the word motion, not a jump
    Given the cursor is at line 3 column 5
    When I jump to "src/filter.rs" line 2
    And I press the key "Alt+Left"
    Then the current buffer is "src/filter.rs"

  Scenario: A jump out of a Preview records the line the row was rendered from
    Given "notes.md" is open in the editor holding:
      """
      # Notes

      Install it.
      Then run it.

      Done here.
      """
    And the cursor is on the row holding "Done here."
    And the Cursor history pane is shown
    When I jump to "src/filter.rs" line 1
    Then the cursor history holds:
      | file          | line |
      | src/editor.rs | 1    |
      | notes.md      | 6    |
    And Cursor history row 2 shows the excerpt "Done here."

  Scenario: A jump inside a Preview records the row's line and comes back to that row
    Given "notes.md" is open in the editor holding:
      """
      # Notes

      Install it.
      Then run it.

      Done here.
      """
    And the cursor is on the row holding "Done here."
    When I jump to "notes.md" line 1
    Then the cursor is on the row holding "Notes"
    And the cursor history holds:
      | file          | line |
      | src/editor.rs | 1    |
      | notes.md      | 6    |
    When I press the key "Ctrl+p"
    Then the current buffer is "notes.md"
    And the cursor is on the row holding "Done here."

  Scenario: A find in a Preview records the line the search was opened from
    Given "notes.md" is open in the editor holding:
      """
      # Notes

      Install it.
      Then run it.

      Done here.
      """
    And the cursor is on the row holding "Done here."
    And I press "/" in the editor
    And I type "Notes" into the in-file search
    When I press Enter during the in-file search
    Then the cursor is on the row holding "Notes"
    And the cursor history holds:
      | file          | line |
      | src/editor.rs | 1    |
      | notes.md      | 6    |

  Scenario: The end of a Preview is a jump, and the place it left is a source line
    Given "notes.md" is open in the editor holding:
      """
      # Notes

      Install it.
      Then run it.

      Done here.
      """
    When I press "G" in the editor
    Then the cursor is on the row holding "Done here."
    And the cursor history holds:
      | file          | line |
      | src/editor.rs | 1    |
      | notes.md      | 1    |

  Scenario: The modifier-free route goes back from a Preview, which reads rather than edits
    Given the cursor is at line 3 column 5
    And "notes.md" is open in the editor holding:
      """
      # Notes
      """
    When I press "p" in the editor
    Then the editor is showing preview
    And the editor refuses with "read-only-preview"
    When I press "gp" in the editor
    Then the current buffer is "src/editor.rs"
    And the cursor is at line 3 column 5

  Scenario: The palette offers the pane in the group the panes live in
    Given the view palette is shown
    Then the view palette offers "Cursor history" in the group "Panes" under the key "y"

  Scenario: Picking the entry shows the pane and puts focus in it
    Given the Cursor history pane is hidden
    And the view palette is shown
    When I click the palette entry "Cursor history"
    Then the Cursor history pane is shown
    And the history pane has focus

  Scenario: Picking it again hides the pane and returns focus to the tree
    Given the Cursor history pane is shown
    And the view palette is shown
    When I click the palette entry "Cursor history"
    Then the Cursor history pane is hidden
    And the file tree pane has focus

  Scenario: Opening it while the Buffers pane is showing replaces it, because they are one slot
    Given the Buffers pane is shown
    When I show the Cursor history pane
    Then the Cursor history pane is shown
    And the Buffers pane is hidden

  Scenario: A row names the file, the line and what the cursor was standing on
    Given the cursor is at line 3 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    Then Cursor history row 1 names the file "editor.rs" on line 3
    And Cursor history row 1 shows the excerpt "pub struct Buffer..."

  Scenario: A row's excerpt is the word the cursor was on and a few more, then an ellipsis
    Given the cursor is at line 1 column 5
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    Then Cursor history row 1 shows the excerpt "std::fmt;"

  Scenario: A row whose line no longer holds what was recorded is not claimed as current
    Given the cursor is at line 1 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    When the line Cursor history row 1 recorded is edited
    Then Cursor history row 1 is stale

  Scenario: A row still holding what it recorded is not marked stale
    Given the cursor is at line 1 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    Then Cursor history row 1 is not stale

  Scenario: The row you are standing on is the one the history's cursor names
    Given the cursor is at line 3 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    When I press the key "Ctrl+p"
    Then the Cursor history selection is row 1

  Scenario: With nothing travelled to, no row is claimed as where you are
    Given the cursor is at line 3 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    Then the Cursor history selection is nothing

  Scenario: Enter on a row goes to that file and line
    Given the cursor is at line 4 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    And the Cursor history selection is row 1
    When I press "Enter"
    Then the current buffer is "src/editor.rs"
    And the cursor is on line 4
    And the editor pane has focus

  Scenario: Enter with nothing travelled to refuses out loud, and not as a way back
    Given the cursor is at line 4 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    Then the Cursor history selection is nothing
    When I press "Enter"
    Then the notice is "no-place-here"
    And the current buffer is "src/filter.rs"

  Scenario: The row offers exactly one action
    Given the cursor is at line 4 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    And the Cursor history selection is row 1
    Then the Cursor history row actions offered are:
      | go-to-place |

  Scenario: A click on a row's icon goes there and leaves the keyboard in the pane
    Given the cursor is at line 4 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    And the Cursor history selection is row 1
    When I click the "go-to-place" action on Cursor history row 1
    Then the current buffer is "src/editor.rs"
    And the cursor is on line 4
    And the history pane has focus

  Scenario: A click on a row goes there and leaves the keyboard in the pane
    Given the cursor is at line 4 column 1
    And I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    When I click Cursor history row 1
    Then the current buffer is "src/editor.rs"
    And the cursor is on line 4
    And the history pane has focus

  Scenario: A Row selection is not text, so it is never copied
    Given the cursor history holds 40 places
    And the Cursor history pane is shown
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
    Given the cursor history holds 40 places
    And the Cursor history selection is row 1
    And the Cursor history pane is shown
    When I scroll down 3 times with the pointer over the history pane
    Then the Cursor history first visible row is row 3
    And the Cursor history selection is row 1

  Scenario: Every event but the wheel pulls the selection back into view
    Given the cursor history holds 40 places
    And the Cursor history selection is row 1
    And the Cursor history pane is shown
    And I scroll down with the pointer over the history pane
    When I press "j"
    Then the Cursor history selection is in view

  Scenario: The history is this session's, and does not survive a restart
    Given I jump to "src/filter.rs" line 1
    And the Cursor history pane is shown
    When Varde starts in the project
    Then the cursor history is empty

  Scenario: The pane's visibility survives a restart
    Given the project ".varde/state.json" records the corner pane as "History"
    When Varde starts in the project
    Then the Cursor history pane is shown
