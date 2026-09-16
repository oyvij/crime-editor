Feature: Hiding the key reminder

  The box of keys in the editor's top-right corner sits over the code, and a terminal
  cell holds one character: there is no opacity to give it, so whatever it covers is
  gone while it is up. `:help` takes it down and puts it back.

  Hiding is remembered per project, the way the last view and the tree divider are: a
  reminder you dismissed should stay dismissed rather than cover the same line again on
  the next launch. The palette offers it too — the box is the only place the `:` commands
  are advertised, so a gesture that just the box tells you about would be unreachable
  once it is down.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the current view is Edit

  Scenario: The key reminder is up to begin with
    Then the key reminder is shown

  Scenario: The help command takes the reminder down
    When I ask CRIME for help from the command line
    Then the key reminder is not shown

  Scenario: The help command puts it back
    Given the key reminder is hidden
    When I ask CRIME for help from the command line
    Then the key reminder is shown

  Scenario: Hiding it is remembered
    When I ask CRIME for help from the command line
    Then the remembered key reminder is "hidden"

  Scenario: A reminder hidden last session is still hidden
    Given the project ".crime/state.json" records the key reminder as hidden
    When CRIME starts in the project
    Then the key reminder is not shown

  Scenario: State recorded before the reminder could be hidden leaves it up
    Given the project ".crime/state.json" records the last view as "Edit"
    When CRIME starts in the project
    Then the key reminder is shown

  Scenario: The palette puts a hidden reminder back
    Given the key reminder is hidden
    And the view palette is shown
    When I press "h"
    Then the key reminder is shown
    And the view palette is not shown
