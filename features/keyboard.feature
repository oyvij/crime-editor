Feature: Working without a mouse

  Focus moves with Alt and a direction, chosen because the terminal pane runs a
  real shell: Tab must stay with completion, and Ctrl+K and Ctrl+L must stay with
  readline. Alt is one of the few things nothing else wants.

  Every one of those is an alias, never the only way: the palette lists every pane,
  and it opens without a modifier. A terminal that rejects the Kitty keyboard flags
  reports no bare Ctrl press at all, and it is the same terminal where Option is not
  Alt — so in a hosted pane there a double-tapped Escape opens the palette instead.
  Both escapes still reach the child, so the way out withholds nothing from it. Where
  the bare Ctrl press is reported, Escape stays the child's alone: it already has a
  gesture, and a CLI that binds Esc Esc itself should keep it.

  Keys go to the pane that has focus. A pane with no key handling yet swallows
  them rather than letting them fall through to the shell — focus means the pane
  receives keys, even when it does nothing with them.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the file tree shows the collapsed folder "src"
    And "src" contains "tree.js"

  Scenario: Alt and a direction moves focus
    Given the editor pane has focus
    When I press "Alt+h"
    Then the file tree pane has focus

  Scenario: Focus moves back the other way
    Given the file tree pane has focus
    When I press "Alt+l"
    Then the editor pane has focus

  Scenario: Focus moves down into the terminal
    Given the editor pane has focus
    When I press "Alt+j"
    Then the terminal pane has focus

  Scenario: The palette moves focus out of a hosted pane without a modifier
    Given the terminal pane has focus
    And the view palette is shown
    When I press "e"
    Then the editor pane has focus

  Scenario: Double-tapping Escape opens the palette from a hosted pane
    Given the terminal does not report modifier key events
    And the double-tap window is 300 ms
    And the terminal pane has focus
    When I press the key "Escape" and press it again after 120 ms
    Then the view palette is shown
    And the terminal received the escape twice

  Scenario: Where a bare Ctrl press is reported, Escape stays the child's alone
    Given the terminal reports modifier key events
    And the double-tap window is 300 ms
    And the terminal pane has focus
    When I press the key "Escape" and press it again after 120 ms
    Then the view palette is not shown
    And the terminal received the escape twice

  Scenario: Two escapes too far apart are not a double-tap
    Given the terminal does not report modifier key events
    And the double-tap window is 300 ms
    And the terminal pane has focus
    When I press the key "Escape" and press it again after 450 ms
    Then the view palette is not shown
    And the terminal received the escape twice

  Scenario: Typing reaches the pane that has focus
    Given the terminal pane has focus
    When I type "ls"
    Then the terminal received "ls"

  Scenario: A paste reaches the focused pane's child as one send
    Given the terminal pane has focus
    When I paste "echo one"
    Then the terminal received "echo one"

  Scenario: A paste into a pane CRIME interprets reaches no child
    Given the editor pane has focus
    When I paste "echo one"
    Then the terminal received nothing

  Scenario: A pane with no key handling swallows what it is given
    Given the editor pane has focus
    When I type "ls"
    Then the terminal received nothing

  Scenario: Arrow keys move the selection in the tree
    Given the file tree pane has focus
    When I press "Down"
    Then the tree selection is "src"

  Scenario: The selection stops at the end of the list
    Given the file tree pane has focus
    And the tree selection is "src"
    When I press "Down"
    Then the tree selection is "src"

  Scenario: Moving onto a file previews it
    Given the folder "src" is expanded
    And the file tree pane has focus
    And the tree selection is "src"
    When I press "Down"
    Then "/home/me/projects/crime/src/tree.js" is open in the editor
    And the file tree pane has focus

  Scenario: Moving onto a folder previews nothing
    Given the file tree pane has focus
    When I press "Down"
    Then no file was opened in the editor

  Scenario: A dirty buffer does not block expanding folders
    Given "notes.md" is open in the editor with unsaved edits
    And the file tree pane has focus
    And the tree selection is "src"
    When I press "Enter"
    Then "src" is expanded

  Scenario: Enter expands the selected folder
    Given the file tree pane has focus
    And the tree selection is "src"
    When I press "Enter"
    Then "src" is expanded

  Scenario: Enter collapses a folder that is already open
    Given the folder "src" is expanded
    And the file tree pane has focus
    And the tree selection is "src"
    When I press "Enter"
    Then "src" is collapsed

  Scenario: Enter opens the selected file and follows it
    Given the folder "src" is expanded
    And the file tree pane has focus
    And the tree selection is "src/tree.js"
    When I press "Enter"
    Then "/home/me/projects/crime/src/tree.js" is open in the editor
    And the editor pane has focus

  Scenario: Expanding a folder keeps you in the tree
    Given the file tree pane has focus
    And the tree selection is "src"
    When I press "Enter"
    Then "src" is expanded
    And the file tree pane has focus
