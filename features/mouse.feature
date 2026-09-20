Feature: The whole TUI is usable with the mouse

  Every pane can be focused by clicking it, and a click both focuses and acts — there is no
  dead first click when moving between panes. Scrolling follows the pointer rather than
  focus, so the terminal can be read while the editor keeps focus.

  The terminal pane is the exception that has to be handled carefully: the shell may be
  running a program that wants mouse events itself. When the running program has enabled
  mouse reporting, clicks inside that pane belong to it — reported in the encoding that
  program asked for, since a report in any other encoding is text it cannot parse and
  leaves behind in its prompt.

  There are no context menus. Every action is reachable from the toolbar and the keyboard.

  Every pane can be scrolled to content past its bottom edge. The wheel never moves focus,
  never moves the tree selection, and never opens a file. In the editor it takes the cursor
  along, but only when the view would otherwise leave it behind — scrolling to a line you
  cannot type on is a caret you have lost.

  The two pty panes scroll their own history, which only the pty can supply, so the core
  asks for it and the edge answers — unless the program running there asked for mouse
  events, in which case the wheel belongs to it. A full-screen program like an AI CLI has
  no history for us to show: it scrolls itself.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the current view is Edit

  Scenario: Clicking a pane focuses it
    Given the editor pane has focus
    When I click in the terminal pane
    Then the terminal pane has focus

  Scenario: Clicking a file opens it but keeps focus where you clicked
    Given the editor pane has focus
    And the file tree shows the file "src/tree.js"
    When I click the row "src/tree.js"
    Then "/home/me/projects/crime/src/tree.js" is open in the editor
    And the file tree pane has focus

  Scenario: A click on an unfocused pane also acts
    Given the editor pane has focus
    And the file tree shows the collapsed folder "src"
    When I click the row "src"
    Then the file tree pane has focus
    And "src" is expanded

  Scenario: Clicking a row selects it, so its actions appear
    Given the file tree shows the collapsed folder "src"
    When I click the row "src"
    Then the tree selection is "src"

  Scenario: Clicking a folder toggles it
    Given the file tree shows the collapsed folder "src"
    When I click the row "src"
    Then "src" is expanded

  Scenario: Clicking an expanded folder collapses it
    Given the file tree shows the expanded folder "src"
    When I click the row "src"
    Then "src" is collapsed

  Scenario: The scroll wheel scrolls the pane under the pointer
    Given the editor pane has focus
    When I scroll down with the pointer over the terminal pane
    Then the terminal pane scrolled down
    And the editor pane did not scroll
    And the editor pane still has focus

  Scenario: Scrolling up over the terminal reaches its history
    When I scroll up with the pointer over the terminal pane
    Then the terminal pane scrolled up

  Scenario: Scrolling up over the AI pane reaches its history
    When I scroll up with the pointer over the AI pane
    Then the AI pane scrolled up

  Scenario: Scrolling the editor takes the cursor with it
    Given the screen is 12 rows by 40 columns
    And "src/long.js" is open in the editor with 12 lines
    When I scroll down with the pointer over the editor pane
    Then the editor view starts at line 2
    And the cursor is at line 2 column 1

  Scenario: A cursor the scroll did not leave behind stays where it is
    Given the screen is 12 rows by 40 columns
    And "src/long.js" is open in the editor with 12 lines
    And the cursor is on line 6
    When I scroll down with the pointer over the editor pane
    Then the editor view starts at line 2
    And the cursor is at line 6 column 1

  Scenario: Scrolling the editor stops at the last screenful
    Given the screen is 12 rows by 40 columns
    And "src/long.js" is open in the editor with 12 lines
    And I scroll down 5 times with the pointer over the editor pane
    And I scroll down with the pointer over the editor pane
    When I scroll down with the pointer over the editor pane
    Then the editor view starts at line 7

  Scenario: Moving the cursor pulls the editor view back to it
    Given the screen is 12 rows by 40 columns
    And "src/long.js" is open in the editor with 12 lines
    And I scroll down with the pointer over the editor pane
    When I press "gg" in the editor
    Then the editor view starts at line 1
    And the cursor is at line 1 column 1

  Scenario: Scrolling the file tree moves the view, not the selection
    Given the screen is 12 rows by 40 columns
    And the workspace folder holds 12 files
    And the tree selection is "file-01.js"
    When I scroll down with the pointer over the file tree pane
    Then the file tree view starts at row 2
    And the tree selection is "file-01.js"
    And no file was opened in the editor

  Scenario: Moving the selection pulls the tree view back to it
    Given the screen is 12 rows by 40 columns
    And the workspace folder holds 12 files
    And the tree selection is "file-01.js"
    And I scroll down with the pointer over the file tree pane
    When I press "Down"
    Then the file tree view starts at row 2
    And the tree selection is "file-02.js"

  Scenario: Right-clicking does nothing
    Given the editor pane has focus
    When I right-click in the editor pane
    Then nothing happened

  Scenario Outline: Clicks reach a program in the encoding it asked for
    Given the <pane> pane has focus
    And the <pane> program asked for "<encoding>" mouse reporting
    When I click in the <pane> pane
    Then the click reached the <pane> program in "<encoding>" encoding

    Examples:
      | pane     | encoding |
      | terminal | SGR      |
      | terminal | legacy   |
      | AI       | SGR      |
      | AI       | legacy   |

  Scenario: Clicks stay ours when the AI CLI did not ask for them
    Given the AI program asked for "no" mouse reporting
    When I click in the AI pane
    Then nothing reached the AI program
    And the AI pane has focus

  Scenario Outline: The wheel reaches a program in the encoding it asked for
    Given the <pane> program asked for "<encoding>" mouse reporting
    When I scroll up with the pointer over the <pane> pane
    Then the scroll reached the <pane> program in "<encoding>" encoding
    And the <pane> pane did not scroll

    Examples:
      | pane     | encoding |
      | terminal | SGR      |
      | terminal | legacy   |
      | AI       | SGR      |
      | AI       | legacy   |

  Scenario: The wheel at a shell prompt scrolls the history instead
    Given the terminal program asked for "no" mouse reporting
    When I scroll up with the pointer over the terminal pane
    Then the terminal pane scrolled up
    And nothing reached the terminal program

  Scenario: Clicks at a shell prompt are handled by CRIME
    Given the terminal pane has focus
    And the terminal program asked for "no" mouse reporting
    When I click in the terminal pane
    Then nothing reached the terminal program

  Scenario: Clicking a link in the terminal with the jump modifier opens it in the browser
    Given the terminal program asked for "SGR" mouse reporting
    And the terminal shows:
      """
      See https://docs.rs/vt100/latest/vt100/ for the grid.
      """
    When I click on "docs.rs" in the terminal pane with the jump modifier held
    Then the browser opens "https://docs.rs/vt100/latest/vt100/"
    And nothing reached the terminal program

  Scenario: The AI pane's links open the same way
    Given the AI program asked for "SGR" mouse reporting
    And the AI session shows:
      """
      Read https://crates.io/crates/vt100.
      """
    When I click on "crates.io" in the AI pane with the jump modifier held
    Then the browser opens "https://crates.io/crates/vt100"
    And nothing reached the AI program

  Scenario: Clicking plain text with the jump modifier opens nothing
    Given the terminal program asked for "SGR" mouse reporting
    And the terminal shows:
      """
      $ cargo test
      """
    When I click on "cargo" in the terminal pane with the jump modifier held
    Then the browser opens nothing
    And nothing reached the terminal program

  Scenario: Dragging a pane divider resizes the panes
    Given the divider between the file tree and the editor is at column 30
    When I drag that divider to column 40
    Then the divider between the file tree and the editor is at column 40

  Scenario: Pane sizes are remembered per project
    Given the divider between the file tree and the editor is at column 30
    And I drag that divider to column 40
    When CRIME starts in the project
    Then the divider between the file tree and the editor is at column 40

  Scenario: Dragging the AI pane's edge resizes it
    Given the screen is 26 rows by 120 columns
    When I drag the AI pane's edge to column 80
    Then the AI pane is 40 columns wide

  Scenario: The AI pane's width is remembered per project
    Given the screen is 26 rows by 120 columns
    And I drag the AI pane's edge to column 80
    When CRIME starts in the project
    Then the AI pane is 40 columns wide

  Scenario: Dragging selects text and copying puts it on the clipboard
    Given "src/tree.js" is open in the editor holding:
      """
      run("unquoted path")
      """
    When I drag across "unquoted path" in the editor pane
    And I copy the selection
    Then the clipboard holds "unquoted path"

  Scenario: Clicking in the editor puts the cursor where I clicked
    Given "src/tree.js" is open in the editor holding:
      """
      one two
      three four
      """
    When I click at line 2 column 5 in the editor
    Then the cursor is at line 2 column 5
    And the editor pane has focus

  Scenario: Clicking past the end of a line puts the cursor at its end
    Given the minimap is turned off
    And "src/tree.js" is open in the editor holding:
      """
      one two
      three four
      """
    When I click at line 1 column 40 in the editor
    Then the cursor is at line 1 column 7

  Scenario: Clicking clears the selection
    Given "src/tree.js" is open in the editor holding:
      """
      one two
      three four
      """
    And I hold shift and press the Right arrow in the editor
    When I click at line 2 column 1 in the editor
    Then the selection holds nothing

  Rule: The wheel scrolls sideways as well as up and down

    A trackpad swipe sideways is a wheel report like any other, and every surface
    that has a horizontal offset answers it: a Source buffer, a Preview, a diff
    and the code a Story walk is showing. It is the same promise the vertical
    wheel makes one axis over — a pane whose text runs past its right border is a
    pane you can read to the end of without moving the caret onto that line.

    Where the surface has a cursor the offset follows, the swipe takes the cursor
    with it, exactly as scrolling down does: a caret off the left of the screen is
    a caret you have lost. Which is also what bounds it — on a Source buffer or a
    Preview the caret cannot leave the line it is on, so a swipe over a short line
    lasts until the next event and no longer, while the read-only surfaces, having
    no caret to be pulled back to, keep it. Sliding right stops at the widest line
    the surface holds rather than running on into empty space, and a pty pane
    whose child asked for mouse events is handed the report instead, in its own
    encoding.

    Scenario: Swiping right slides a Source buffer sideways
      Given the screen is 12 rows by 80 columns
      And the minimap is turned off
      And "src/long.js" is open in the editor holding a 40-character line above a short one
      When I scroll right with the pointer over the editor pane
      Then the editor view starts at column 9

    Scenario: Swiping right takes the cursor with it
      Given the screen is 12 rows by 80 columns
      And the minimap is turned off
      And "src/long.js" is open in the editor holding a 40-character line above a short one
      When I scroll right with the pointer over the editor pane
      Then the cursor is at line 1 column 9

    Scenario: Swiping left brings a slid buffer back
      Given the screen is 12 rows by 80 columns
      And the minimap is turned off
      And "src/long.js" is open in the editor holding a 40-character line above a short one
      And I scroll right 2 times with the pointer over the editor pane
      When I scroll left with the pointer over the editor pane
      Then the editor view starts at column 9

    Scenario: A swipe leaves the caret on the text of the line it is on
      Given the screen is 12 rows by 80 columns
      And the minimap is turned off
      And "src/long.js" is open in the editor holding a 40-character line above a short one
      And the cursor is on line 2
      When I scroll right with the pointer over the editor pane
      Then the editor view starts at column 9
      And the cursor is at line 2 column 3

    Scenario: The next keypress pulls a swiped Source buffer back to its caret
      Given the screen is 12 rows by 80 columns
      And the minimap is turned off
      And "src/long.js" is open in the editor holding a 40-character line above a short one
      And the cursor is on line 2
      And I scroll right with the pointer over the editor pane
      When I press "j" in the editor
      Then the editor view starts at column 1

    Scenario: A swipe leaves the view where the vertical wheel put it
      Given the screen is 12 rows by 80 columns
      And the minimap is turned off
      And "src/long.js" is open in the editor holding:
        """
        xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
        two
        three
        four
        five
        six
        seven
        eight
        nine
        ten
        eleven
        twelve
        """
      And I scroll down 3 times with the pointer over the editor pane
      When I scroll right with the pointer over the editor pane
      Then the editor view starts at line 4
      And the editor view starts at column 9

    Scenario: Swiping right slides a diff
      Given the screen is 12 rows by 80 columns
      And the diff for "src/tree.js" is shown holding a 60-character line
      When I scroll right with the pointer over the editor pane
      Then the editor view starts at column 9

    Scenario: Swiping right past the widest line stops there
      Given the screen is 12 rows by 80 columns
      And the diff for "src/tree.js" is shown holding a 24-character line
      When I scroll right 4 times with the pointer over the editor pane
      Then the editor view starts at column 11

    Scenario: Swiping left at the home column stays home
      Given the screen is 12 rows by 80 columns
      And the diff for "src/tree.js" is shown holding a 60-character line
      When I scroll left with the pointer over the editor pane
      Then the editor view starts at column 1

    Scenario: A swipe over the file tree slides nothing
      Given the screen is 12 rows by 80 columns
      And the workspace folder holds 12 files
      When I scroll right with the pointer over the file tree pane
      Then the file tree view starts at row 1
      And the editor view starts at column 1

    Scenario Outline: A sideways swipe reaches a program in the encoding it asked for
      Given the <pane> program asked for "<encoding>" mouse reporting
      When I scroll right with the pointer over the <pane> pane
      Then the scroll reached the <pane> program in "<encoding>" encoding
      And the <pane> pane did not scroll

      Examples:
        | pane     | encoding |
        | terminal | SGR      |
        | terminal | legacy   |
        | AI       | SGR      |
        | AI       | legacy   |

    Scenario: A sideways swipe at a shell prompt reaches nobody
      Given the terminal program asked for "no" mouse reporting
      When I scroll right with the pointer over the terminal pane
      Then nothing reached the terminal program
