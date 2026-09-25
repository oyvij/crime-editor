Feature: Variables and Watches

  The Variables are the Debug adapter's tree for the chosen Frame, grouped by scope and expanded one
  level at a time by reference, so walking a deep structure asks for what is opened and nothing
  else. Large collections are paged. How a member is drawn — private, read-only, lazy — follows the
  adapter's presentation hints, so the tree looks the same in every language without Varde knowing
  which one it is.

  The focused row carries its actions as Chips — set value, copy, watch, evaluate, ask AI — each also
  a key. An action the adapter cannot do is dimmed, never hidden, so the user learns it exists.

  Watches sit at the top and are re-evaluated at every pause. One that calls something is marked as
  calling, since it runs that call again at every pause — decided from the syntax tree, never by
  asking the adapter to try it.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a Debug adapter for "rust" is configured
    And "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let orders = load();
          let count = orders.len();
          println!("{}", count);
      }
      """

  Rule: The Variables are the adapter's tree, grouped by scope and opened one level at a time

    Scenario: The Variables are grouped by the adapter's scopes
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter answers "scopes" with the scopes:
        | Locals  |
        | Statics |
      Then the Variables' top rows are:
        | Locals  |
        | Statics |

    Scenario: Opening a row asks the adapter for its children by reference
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "orders" with reference 7
      When I open the Variables row "orders"
      Then the Debug adapter was sent a "variables" request for reference 7

    Scenario: Opening a row asks for one level only
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "orders" with reference 7
      When I open the Variables row "orders"
      Then the Debug adapter was sent exactly 1 "variables" request since the pause

    Scenario: A closed row asks for nothing
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Variables show "orders" with reference 7
      Then the Debug adapter was sent no "variables" request for reference 7

    Scenario: A large collection is paged
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "orders" with reference 7 holding 10000 indexed children
      When I open the Variables row "orders"
      Then the Debug adapter was sent a "variables" request for reference 7 starting at 0 counting 100
      And the Variables show a row for the next page of "orders"

    Scenario: Opening the next page asks for it
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "orders" with reference 7 holding 10000 indexed children
      And I open the Variables row "orders"
      When I open the next page of "orders"
      Then the Debug adapter was sent a "variables" request for reference 7 starting at 100 counting 100

    Scenario Outline: Presentation hints decide how a member is drawn
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Variables show "secret" with the presentation hint "<hint>"
      Then the Variables row "secret" is drawn as "<drawn>"

      Examples:
        | hint     | drawn     |
        | private  | private   |
        | readOnly | read-only |
        | lazy     | lazy      |

    Scenario: A lazy member is fetched only when opened
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "total" with reference 9 and the presentation hint "lazy"
      Then the Debug adapter was sent no "variables" request for reference 9

  Rule: The focused row carries its actions as Chips, each also a key, dimmed never hidden

    Scenario: The focused row shows its Chips
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables have focus
      When I move the Variables selection to the row "count"
      Then the row "count" carries the Chips:
        | set-value |
        | copy      |
        | watch     |
        | evaluate  |
        | ask-ai    |

    Scenario: A row that is not focused carries no Chips
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables selection is the row "count"
      Then the row "orders" carries no Chips

    Scenario: Setting a value sends it as written
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Debug adapter reported it can set variables
      And the Variables selection is the row "count"
      When I set the value of the row to "count + 1"
      Then the Debug adapter was sent a "setVariable" request for "count" with the value "count + 1"

    Scenario: The adapter's new value replaces the row's
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Debug adapter reported it can set variables
      And the Variables selection is the row "count"
      And I set the value of the row to "4"
      When the Debug adapter answers "setVariable" with the value "4"
      Then the Variables row "count" shows the value "4"

    Scenario: Set value is dimmed where the adapter cannot set variables
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Debug adapter reported it cannot set variables
      When I move the Variables selection to the row "count"
      Then the "set-value" Chip is dimmed
      And the row "count" carries the Chips:
        | set-value |
        | copy      |
        | watch     |
        | evaluate  |
        | ask-ai    |

    Scenario: A refused set value says why
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Debug adapter reported it can set variables
      And the Variables selection is the row "count"
      And I set the value of the row to "\"three\""
      When the Debug adapter answers "setVariable" with the error "mismatched types"
      Then the editor refuses with "set-value-failed"

    Scenario: Copying a row copies its value
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables row "count" shows the value "3"
      And the Variables selection is the row "count"
      When I click the "copy" Chip
      Then the clipboard holds "3"

    Scenario: Copying a row as an expression copies the path to it
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables row "orders[0].id" is open
      And the Variables selection is the row "orders[0].id"
      When I copy the row as an expression
      Then the clipboard holds "orders[0].id"

    Scenario: The evaluate Chip opens the Evaluator on the row's expression
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables selection is the row "count"
      When I click the "evaluate" Chip
      Then the Evaluator is open holding "count"

  Rule: Watches sit on top and are re-evaluated at every pause

    Scenario: The watch Chip adds the row as a Watch
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables selection is the row "count"
      When I click the "watch" Chip
      Then the Watches are:
        | count |

    Scenario: Watches are the Variables' top rows
      Given the Watches are "orders.len()" and "count"
      When a Debug session is Paused at "src/main.rs" line 3
      Then the Variables' first rows are the Watches

    Scenario: Every pause re-evaluates every Watch in the watch context
      Given the Watches are "orders.len()" and "count"
      And a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "step"
      Then the Debug adapter was sent an "evaluate" request for "orders.len()" in the "watch" context
      And the Debug adapter was sent an "evaluate" request for "count" in the "watch" context

    Scenario: A Watch that calls something is marked as calling
      Given the Watches are "orders.len()" and "count"
      When a Debug session is Paused at "src/main.rs" line 3
      Then the Watch "orders.len()" is marked as calling
      And the Watch "count" is not marked as calling

    Scenario: A Watch the adapter cannot evaluate shows the adapter's reason
      Given the Watches are "missing"
      And a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter answers the "evaluate" for "missing" with the error "cannot find value"
      Then the Watch "missing" shows an error

    Scenario: A Watch is removed from its row
      Given the Watches are "orders.len()" and "count"
      And a Debug session is Paused at "src/main.rs" line 3
      And the Variables selection is the Watch "count"
      When I click the "remove-watch" Chip
      Then the Watches are:
        | orders.len() |

    Scenario: Watches outlive the session
      Given the Watches are "count"
      And a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter sends the event "terminated"
      Then the Watches are:
        | count |
