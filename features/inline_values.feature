Feature: Inline values

  While Paused, a variable's value is drawn faintly at the end of a line the Paused call has already
  run, in the Debug adapter's own words, so state can be read without leaving the code. Never on a
  line the call has not reached: a value shown there would be one from before it was assigned, which
  is worse than none.

  A value that changed since the last pause is highlighted for that pause, which is what makes
  stepping show what the line just did. Inline values are trimmed to the width the line leaves and
  never push code off screen.

  What is asserted is which lines carry which values and which are highlighted — never the faint
  colour itself.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a Debug adapter for "rust" is configured
    And "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let total = 0;
          let count = 3;
          let sum = total + count;
          println!("{}", sum);
      }
      """

  Rule: Values are drawn only on lines the Paused call has already run

    Scenario: A line already run carries its values
      Given the Debug adapter's Locals are "total" = "0" and "count" = "3"
      When a Debug session is Paused at "src/main.rs" line 4
      Then line 2 carries the Inline value "total" = "0"
      And line 3 carries the Inline value "count" = "3"

    Scenario: The Paused line and the lines after it carry none
      Given the Debug adapter's Locals are "total" = "0" and "count" = "3"
      When a Debug session is Paused at "src/main.rs" line 3
      Then line 3 carries no Inline value
      And line 4 carries no Inline value
      And line 2 carries the Inline value "total" = "0"

    Scenario: Lines outside the Paused call carry none
      Given the Debug adapter's Locals are "total" = "0" and "count" = "3"
      And "src/lib.rs" is open in the editor holding:
        """
        pub fn total() -> u32 { 0 }
        """
      When a Debug session is Paused at "src/main.rs" line 4
      Then "src/lib.rs" carries no Inline values

    Scenario: While Running the Inline values stay, dimmed
      Given a Debug session is Paused at "src/main.rs" line 4
      When the Debug adapter reports the program continued
      Then the Inline values are drawn dimmed

    Scenario: No Inline values once the session ends
      Given a Debug session is Paused at "src/main.rs" line 4
      When the Debug adapter sends the event "terminated"
      Then no line carries an Inline value

    Scenario: Choosing another Frame draws that call's values
      Given a Debug session is Paused in "add" at "src/lib.rs" line 8 called from "main" at "src/main.rs" line 4
      And the Debug adapter's Locals for "main" are "total" = "0" and "count" = "3"
      When I choose the Frame "main"
      Then line 3 carries the Inline value "count" = "3"

  Rule: A value that changed since the last pause is highlighted for that pause

    Scenario: A value that changed is highlighted
      Given a Debug session is Paused at "src/main.rs" line 3 with "total" = "0"
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "step" and "total" = "0" and "count" = "3"
      Then the Inline value "count" is highlighted
      And the Inline value "total" is not highlighted

    Scenario: The highlight lasts one pause
      Given a Debug session is Paused at "src/main.rs" line 3 with "total" = "0"
      And the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "step" and "total" = "0" and "count" = "3"
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 5 with reason "step" and "total" = "0" and "count" = "3"
      Then the Inline value "count" is not highlighted

  Rule: Inline values are trimmed to the width the line leaves

    Scenario: A long value is trimmed to the remaining width
      Given the editor is 40 columns wide
      And the Debug adapter's Locals are "total" = "a value far longer than the forty columns the editor has left"
      When a Debug session is Paused at "src/main.rs" line 4
      Then line 2's Inline values end within the editor's width

    Scenario: A line that fills the width carries no Inline value
      Given the editor is 16 columns wide
      And the Debug adapter's Locals are "count" = "3"
      When a Debug session is Paused at "src/main.rs" line 4
      Then line 3 carries no Inline value
