Feature: Resting the pointer says what a symbol is

  `K` already asks the Language server what the symbol under the *cursor* is. A reader whose hand is
  on the mouse has to move the cursor there first, which is a keystroke and a lost place, so the
  same question is asked by resting the pointer on the symbol for half a second.

  The rest is the whole of the gesture: a pointer crossing the pane on its way somewhere else asks
  nothing, and the window is restarted by every cell it crosses. The edge holds the timer and
  nothing else — how long to rest is the core's number, carried by the effect, exactly as the
  Candidate list's debounce is, so no scenario waits on a real clock.

  What the box describes is where the *pointer* is, not where the cursor is. That is the one thing
  the keystroke's rule cannot cover: a hover asked for by the cursor is stale the moment the cursor
  moves, and a hover asked for by the pointer stands while the pointer is still on it and goes when
  it leaves. Nothing moves the cursor — a caret that follows the mouse would rewrite the next
  keystroke's place in insert mode, which is the reason the gesture asks rather than clicks.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And the project is a git repository

  Rule: A pointer that rests on a symbol asks what it is, in either mode

    Scenario: Resting the pointer asks the server about the symbol under it
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor
      When the pointer rests on line 1 column 4 in the editor
      Then the language server for "rust" was sent a "textDocument/hover" request for "src/lib.rs" line 1 column 4

    Scenario: Resting the pointer while inserting asks the same question
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor
      And I press "i" in the editor
      When the pointer rests on line 1 column 4 in the editor
      Then the language server for "rust" was sent a "textDocument/hover" request for "src/lib.rs" line 1 column 4

    Scenario: A pointer crossing the pane asks nothing until it has rested
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor
      When the pointer moves to line 1 column 4 in the editor
      Then the pointer must rest 500 ms before the server is asked
      And the language server for "rust" was sent no "textDocument/hover" request

    Scenario: The cursor stays where it was
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor
      And the cursor is at line 1 column 1
      When the pointer rests on line 1 column 4 in the editor
      Then the cursor is at line 1 column 1

  Rule: The box describes where the pointer is, and goes when the pointer leaves

    Scenario: The reply is shown even though the cursor is elsewhere
      Given a language server for "rust" is ready
      And "src/lib.rs" holds:
        """
        fn main() {
            let one = 1;
        }
        """
      And "src/lib.rs" is open in the editor
      And the cursor is at line 1 column 1
      And the pointer rests on line 2 column 9 in the editor
      When the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"let one: i32"}}
        """
      Then the hover shows "let one: i32"

    Scenario: The pointer moving on takes the box down
      Given a language server for "rust" is ready
      And "src/lib.rs" holds:
        """
        fn main() {
            let one = 1;
        }
        """
      And "src/lib.rs" is open in the editor
      And the pointer rests on line 2 column 9 in the editor
      And the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"let one: i32"}}
        """
      When the pointer moves to line 1 column 1 in the editor
      Then no hover is shown

    Scenario: The pointer leaving the editor takes the box down
      Given a language server for "rust" is ready
      And "src/lib.rs" holds:
        """
        fn main() {
            let one = 1;
        }
        """
      And "src/lib.rs" is open in the editor
      And the pointer rests on line 2 column 9 in the editor
      And the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"let one: i32"}}
        """
      When the pointer moves out of the editor
      Then no hover is shown

  Rule: The box stands while the pointer is on the symbol or on the box itself

    Scenario: Moving the pointer onto the box keeps it up
      Given a language server for "rust" is ready
      And "src/lib.rs" holds:
        """
        fn main() {
            let one = 1;
        }
        """
      And "src/lib.rs" is open in the editor
      And the pointer rests on line 2 column 9 in the editor
      And the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"let one: i32"}}
        """
      When the pointer moves onto the hover
      Then the hover shows "let one: i32"

    Scenario: Leaving the box for somewhere else takes it down
      Given a language server for "rust" is ready
      And "src/lib.rs" holds:
        """
        fn main() {
            let one = 1;
        }
        """
      And "src/lib.rs" is open in the editor
      And the pointer rests on line 2 column 9 in the editor
      And the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"let one: i32"}}
        """
      And the pointer moves onto the hover
      When the pointer moves to line 1 column 1 in the editor
      Then no hover is shown

    Scenario: The wheel over the box scrolls the box, not the editor
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor with 40 lines
      And the pointer rests on line 2 column 9 in the editor
      And the language server for "rust" answers the hover with a 30-line reply
      And the pointer moves onto the hover
      When I scroll down with the pointer over the hover
      Then the hover starts at its row 2
      And the editor view starts at line 1

  Rule: Pressing K again moves the keyboard into the box

    Scenario: A second K moves the keyboard into the hover
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor
      And I press "K" in the editor
      And the language server for "rust" answers the hover with:
        """
        {"contents":{"kind":"plaintext","value":"fn main()"}}
        """
      When I press "K" in the editor
      Then the hover has focus

    Scenario: Escape in the hover gives the keyboard back to the editor
      Given a language server for "rust" is ready
      And "src/lib.rs" is open in the editor
      And the hover has focus
      When I press "Escape"
      Then no hover is shown
      And the editor pane has focus
