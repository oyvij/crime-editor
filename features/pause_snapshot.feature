Feature: Asking the AI about a Paused program

  One press hands the AI session a Pause snapshot: the Paused line and its neighbours, the Frames,
  the Variables as shown, and the exception if one paused it. It is pasted into the AI's prompt and
  never submitted. Variable values can be real customer data, so what leaves the machine is the
  user's call, made by pressing Enter themselves — that absence is asserted in every scenario here
  that pastes.

  The snapshot goes the way the AI hand-off already goes for a review: bracketed as a paste when the
  AI CLI asked for that, held until the CLI is ready, and with no trailing Enter.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a Debug adapter for "rust" is configured
    And "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let total = 0;
          let count = 3;
          println!("{}", total + count);
      }
      """

  Rule: One press pastes a Pause snapshot, and nothing is submitted

    Background:
      Given an AI session is running in the AI pane
      And the AI program asked for bracketed paste

    Scenario: Space then a pastes a Pause snapshot into the AI's prompt
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Space"
      And I press "a"
      Then the AI pane's prompt holds a Pause snapshot
      And the prompt was not submitted to the AI

    Scenario: The ask-AI Chip does the same
      Given a Debug session is Paused at "src/main.rs" line 3
      When I click the "ask-ai" Chip
      Then the AI pane's prompt holds a Pause snapshot
      And the prompt was not submitted to the AI

    Scenario: The snapshot holds the Paused line and its neighbours
      Given a Debug session is Paused at "src/main.rs" line 3
      When I click the "ask-ai" Chip
      Then the Pause snapshot holds "src/main.rs" lines 1 to 5 with line 3 marked as the Paused line

    Scenario: The snapshot holds the Frames and the Variables as shown
      Given a Debug session is Paused in "add" at "src/lib.rs" line 8 called from "main" at "src/main.rs" line 4
      And the Variables show "count" with the value "3"
      When I click the "ask-ai" Chip
      Then the Pause snapshot names the Frames "add" and "main"
      And the Pause snapshot holds the Variable "count" with the value "3"

    Scenario: The snapshot holds only the Variables as shown, not rows left closed
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "orders" with reference 7
      When I click the "ask-ai" Chip
      Then the Debug adapter was sent no "variables" request for reference 7

    Scenario: An exception pause puts the exception in the snapshot
      Given a Debug session is Running
      And the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "exception" and the text "attempt to add with overflow"
      When I click the "ask-ai" Chip
      Then the Pause snapshot holds the exception "attempt to add with overflow"

    Scenario: The snapshot is pasted as a paste when the AI asked for one
      Given a Debug session is Paused at "src/main.rs" line 3
      When I click the "ask-ai" Chip
      Then the AI program received the Pause snapshot as a bracketed paste

    Scenario: The keyboard stays where it was
      Given a Debug session is Paused at "src/main.rs" line 3
      And the editor pane has focus
      When I click the "ask-ai" Chip
      Then the editor pane still has focus

  Rule: Asking about one Variables row asks about that value

    Background:
      Given an AI session is running in the AI pane
      And the AI program asked for bracketed paste

    Scenario: The row's ask-AI Chip pastes that row
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "count" with the value "3"
      And the Variables selection is the row "count"
      When I click the row's "ask-ai" Chip
      Then the AI pane's prompt holds the Variable "count" with the value "3"
      And the prompt was not submitted to the AI

  Rule: Asking needs an AI session and a pause

    Scenario: With no AI session, asking starts the configured one and the snapshot waits for it
      Given no AI session is running in the AI pane
      And the effective setting "ai.command" is "claude"
      And a Debug session is Paused at "src/main.rs" line 3
      When I click the "ask-ai" Chip
      Then an AI session was started with "claude"
      And no prompt was sent to the AI

    Scenario: The waiting snapshot lands once the AI is ready, still unsubmitted
      Given no AI session is running in the AI pane
      And a Debug session is Paused at "src/main.rs" line 3
      And I click the "ask-ai" Chip
      When the AI session is ready for input
      Then the AI pane's prompt holds a Pause snapshot
      And the prompt was not submitted to the AI

    Scenario: The ask-AI Chip is dimmed while the program runs
      Given an AI session is running in the AI pane
      And a Debug session is Running
      Then the "ask-ai" Chip is dimmed
