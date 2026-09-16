Feature: The AI pane

  The AI pane runs whatever CLI `ai.command` names. Submitting a review starts
  one if none is running, but that cannot be the only way in — you should be
  able to just start it and talk to it.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the effective setting "ai.command" is "claude"

  Scenario: With nothing running, the pane asks which CLI to start
    Given no AI session is running in the AI pane
    Then the AI pane is asking which CLI to start

  Scenario: Once a session runs, the pane stops asking
    Given an AI session is running in the AI pane
    Then the AI pane is not asking which CLI to start

  Scenario: When the AI exits, the pane asks again
    Given an AI session is running in the AI pane
    When the AI session exits
    Then the AI pane is asking which CLI to start

  Scenario: A new CLI can be started after the old one exits
    Given an AI session is running in the AI pane
    And the AI session exits
    When I start the AI with "opencode"
    Then an AI session was started with "opencode"

  Scenario: A CLI that cannot be started leaves the pane asking again
    Given no AI session is running in the AI pane
    And the AI CLI cannot be started
    When I start the AI with "nosuchcli"
    Then the AI pane is asking which CLI to start

  Scenario: Keys do not reach a session that never started
    Given no AI session is running in the AI pane
    And the AI CLI cannot be started
    And I start the AI with "nosuchcli"
    And the AI pane has focus
    When I type "hello"
    Then the AI received nothing
    And the terminal received nothing

  Scenario: Another CLI can be started after one failed to start
    Given no AI session is running in the AI pane
    And the AI CLI cannot be started
    And I start the AI with "nosuchcli"
    And the AI CLI can be started
    When I start the AI with "opencode"
    Then the AI was started again with "opencode"
    And the reviewer is not told the AI is already running

  Scenario: Typing with no session running does not reach the shell
    Given no AI session is running in the AI pane
    And the AI pane has focus
    When I type "claude"
    Then the terminal received nothing

  Scenario: Starting the AI runs the configured command
    Given no AI session is running in the AI pane
    When I start the AI
    Then an AI session was started with "claude"
    And the AI pane has focus

  Scenario: Starting the AI when one is running just focuses it
    Given an AI session is running in the AI pane
    When I start the AI
    Then no new AI session was started
    And the AI pane has focus

  Scenario: Starting a named CLI runs that one
    Given no AI session is running in the AI pane
    When I start the AI with "opencode"
    Then an AI session was started with "opencode"
    And the AI pane has focus

  Scenario: The chosen CLI is remembered for next time
    Given no AI session is running in the AI pane
    When I start the AI with "opencode"
    Then the remembered AI command is "opencode"

  Scenario: Starting a different CLI while one runs is refused
    Given an AI session is running in the AI pane
    When I start the AI with "opencode"
    Then no new AI session was started
    And the reviewer is told the AI is already running

  Scenario: Forcing replaces the running session
    Given an AI session is running in the AI pane
    When I force the AI to "opencode"
    Then the AI session was stopped
    And an AI session was started with "opencode"

  Scenario: A submitted review goes to whichever CLI is running
    Given the project is a git repository
    And "src/tree.js" is changed at revision "a3f9c1"
    And no AI session is running in the AI pane
    And I start the AI with "opencode"
    And I add an ISSUE on "src/tree.js" lines 1 to 2 saying "unquoted path"
    And I submit the review
    And I confirm the submission
    When the AI session is ready for input
    Then only one AI session was started
    And the prompt was submitted to the AI

  Scenario: Submitting with nothing running starts the remembered CLI
    Given the project is a git repository
    And "src/tree.js" is changed at revision "a3f9c1"
    And the remembered AI command is "opencode"
    And no AI session is running in the AI pane
    And I add an ISSUE on "src/tree.js" lines 1 to 2 saying "unquoted path"
    And I submit the review
    When I confirm the submission
    Then an AI session was started with "opencode"

  Scenario: Typing goes to the AI once it has focus
    Given an AI session is running in the AI pane
    And I start the AI
    When I type "hello"
    Then the AI received "hello"

  Scenario: Option+Enter reaches the session with its modifier intact
    Given an AI session is running in the AI pane
    And the AI pane has focus
    When I press the key "Alt+Enter"
    Then the AI received the bytes "\e\r"

  Scenario: A multi-line paste reaches the session as one paste, marked as one
    Given an AI session is running in the AI pane
    And the AI pane has focus
    And the AI program asked for bracketed paste
    When I paste "first\nsecond"
    Then the AI received the bytes "\e[200~first\nsecond\e[201~"

  Scenario: A session that did not ask for paste markers gets the text bare
    Given an AI session is running in the AI pane
    And the AI pane has focus
    When I paste "first\nsecond"
    Then the AI received the bytes "first\nsecond"

  Scenario: A paste cannot smuggle the end of its own bracketing
    Given an AI session is running in the AI pane
    And the AI pane has focus
    And the AI program asked for bracketed paste
    When I paste "safe\e[201~rm -rf /"
    Then the AI received the bytes "\e[200~saferm -rf /\e[201~"

  Scenario: Pasting with no session running does not reach the shell
    Given no AI session is running in the AI pane
    And the AI pane has focus
    When I paste "claude"
    Then the terminal received nothing
    And the AI received nothing

  Scenario: A reserved key moves focus rather than reaching the session
    Given an AI session is running in the AI pane
    And the AI pane has focus
    When I press the key "Alt+h"
    Then the AI received nothing
    And the editor pane has focus

  Rule: The pane can take the whole height down the right-hand edge

    Beside the editor with the terminal spanning the width beneath it is the
    default and the right one most of the time. But an AI CLI is a long
    conversation, and reading it two rows at a time is the wrong shape: the
    pane can take the whole height instead, and the terminal gives up the
    width rather than the AI pane giving up rows.

    Scenario: The AI pane takes the whole height
      Given the screen is 26 rows by 120 columns
      When I make the AI pane tall from the command line
      Then the AI pane spans the whole height
      And the terminal pane ends where the AI pane starts

    Scenario: The shape is reachable from the pane it changes
      Given the screen is 26 rows by 120 columns
      And an AI session is running in the AI pane
      And the AI pane has focus
      And the view palette is shown
      When I press the key "l"
      Then the AI pane spans the whole height
      And the AI pane has focus
      And the AI received nothing

    Scenario: Asking again puts it back beside the editor
      Given the screen is 26 rows by 120 columns
      And I make the AI pane tall from the command line
      When I make the AI pane tall from the command line
      Then the AI pane stops above the terminal
      And the terminal pane spans the whole width

    Scenario: A tall pane keeps the width its edge was dragged to
      Given the screen is 26 rows by 120 columns
      And I drag the AI pane's edge to column 80
      When I make the AI pane tall from the command line
      Then the AI pane is 40 columns wide
      And the AI pane spans the whole height

    Scenario: A tall pane is remembered per project
      Given the screen is 26 rows by 120 columns
      And I make the AI pane tall from the command line
      When CRIME starts in the project
      Then the AI pane spans the whole height
