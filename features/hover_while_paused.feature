Feature: A Hover while Paused

  While a Debug session is Paused, a Hover answers what the expression under it holds as well as
  what it is: the value first, as a tree that opens like the Variables, above the Language server's
  type and docs. The expression it describes comes from the syntax tree and is highlighted in the
  editor, so the user knows exactly what was evaluated.

  A Hover never calls anything. An expression that contains a call is not sent to the Debug adapter
  at all — the box says it would have to be evaluated to be known, and offers the Evaluate Chip.
  A pointer passing over `delete_order()` on its way somewhere else must never run it; that absence
  is the most important assertion in this file.

  The box carries Evaluate and Watch Chips, so looking at a value and working with it are one press
  apart.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a Debug adapter for "rust" is configured
    And a language server for "rust" is ready
    And "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let order = load(7);
          delete_order(order.id);
          println!("{}", order.total);
      }
      """
    And a Debug session is Paused at "src/main.rs" line 3

  Rule: A Hover while Paused shows the value first, and highlights what it describes

    Scenario: Resting on a variable asks the adapter in the hover context
      When the pointer rests on line 2 column 9 in the editor
      Then the Debug adapter was sent an "evaluate" request for "order" in the "hover" context

    Scenario: Resting on a field asks for the whole expression the syntax tree names
      When the pointer rests on line 4 column 26 in the editor
      Then the Debug adapter was sent an "evaluate" request for "order.total" in the "hover" context

    Scenario: The value comes before the type and docs
      Given the pointer rests on line 2 column 9 in the editor
      When the Debug adapter answers the "evaluate" for "order" with the value "Order { id: 7, total: 30 }"
      And the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"let order: Order"}}
        """
      Then the hover's first section is the value
      And the hover's second section is the type and docs

    Scenario: The value opens like the Variables
      Given the pointer rests on line 2 column 9 in the editor
      And the Debug adapter answers the "evaluate" for "order" with reference 12
      When I open the hover's value
      Then the Debug adapter was sent a "variables" request for reference 12

    Scenario: The expression the Hover describes is highlighted
      Given the pointer rests on line 4 column 26 in the editor
      When the Debug adapter answers the "evaluate" for "order.total" with the value "30"
      Then the editor highlights "order.total" on line 4

    Scenario: While Running a Hover asks the adapter nothing
      Given the Debug adapter reports the program continued
      When the pointer rests on line 2 column 9 in the editor
      Then the Debug adapter was sent no "evaluate" request

    Scenario: With no session a Hover asks the adapter nothing
      Given the Debug adapter sends the event "terminated"
      When the pointer rests on line 2 column 9 in the editor
      Then the Debug adapter was sent no "evaluate" request

  Rule: A Hover never calls anything

    Scenario: A Hover over a call sends no evaluate request
      When the pointer rests on line 3 column 5 in the editor
      Then the Debug adapter was sent no "evaluate" request

    Scenario: A Hover over an argument inside a call still sends none for the call
      When the pointer rests on line 2 column 22 in the editor
      Then the Debug adapter was sent no "evaluate" request for "load(7)"

    Scenario: A Hover over a call says it must be evaluated to be known
      When the pointer rests on line 3 column 5 in the editor
      Then the hover's value section says "needs-evaluate"
      And the hover carries the "evaluate" Chip

    Scenario: K over a call sends no evaluate request either
      Given the cursor is at line 3 column 5
      When I press "K" in the editor
      Then the Debug adapter was sent no "evaluate" request

  Rule: The Hover's Chips reach the Evaluator and the Watches

    Scenario: The Evaluate Chip opens the Evaluator on the Hover's expression
      Given the pointer rests on line 3 column 5 in the editor
      When I click the hover's "evaluate" Chip
      Then the Evaluator is open holding "delete_order(order.id)"
      And the Debug adapter was sent no "evaluate" request

    Scenario: The Watch Chip adds the Hover's expression as a Watch
      Given the pointer rests on line 4 column 26 in the editor
      When I click the hover's "watch" Chip
      Then the Watches are:
        | order.total |
