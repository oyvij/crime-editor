Feature: Stepping and the Transport

  JetBrains' F-keys drive a Debug session from wherever the keyboard is — even a shell or the
  Program output — because stepping is something done in bursts between reading, and a key that only
  works in one pane is a key that fails the moment focus is elsewhere. They are Reserved keys only
  while a session exists; with none, a hosted pane gets them back.

  Every debug action is also a Space chord, the modifier-free alias `AGENTS.md` requires. A tapped
  Space opens the Chord hint at once, drawn from the same list as the cheatsheet, and a chord leaves
  the keyboard in Stepping mode, where the stepping keys act without Space. Any other key leaves
  Stepping mode and then does what it always does, so nobody is ever trapped in it.

  And every debug action is a Chip on the Variables' Transport (`docs/adr/0022-every-action-has-a-chip.md`):
  a glyph, its F-key and its Space chord, dimmed while unavailable and lit while it was the last
  action taken. Scenarios assert which Chip is dimmed or lit, never the glyphs or the colours.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a Debug adapter for "rust" is configured
    And the project config is:
      """
      [launch.server]
      adapter = "rust"
      request = "launch"
      args = { program = "target/debug/server" }
      """
    And "src/main.rs" is open in the editor holding:
      """
      fn main() {
          let total = 0;
          let count = 3;
          println!("{}", total + count);
      }
      """

  Rule: The JetBrains F-keys step, continue and stop

    Scenario Outline: Each F-key sends its request for the inspected thread
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      When I press "<key>"
      Then the Debug adapter was sent a "<request>" request for thread 1

      Examples:
        | key      | request  |
        | F8       | next     |
        | F7       | stepIn   |
        | Shift+F8 | stepOut  |
        | F9       | continue |

    Scenario: Ctrl+F2 stops the session
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Ctrl+F2"
      Then the Debug adapter was sent a "disconnect" request

    Scenario: F9 while Running pauses the program
      Given a Debug session is Running
      When I press "F9"
      Then the Debug adapter was sent a "pause" request

    Scenario: The F-keys work while a shell has the keyboard
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And terminal 1 has focus
      When I press "F8"
      Then the Debug adapter was sent a "next" request for thread 1
      And nothing reached the terminal program

    Scenario: The F-keys work while the Program output has the keyboard
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And the Program output has focus
      When I press "F8"
      Then the Debug adapter was sent a "next" request for thread 1
      And nothing reached the Program output

    Scenario: With no session the F-keys belong to the shell
      Given no Debug session exists
      And terminal 1 has focus
      When I press "F8"
      Then the terminal program received the key "F8"

    Scenario: Stepping while Running is not sent
      Given a Debug session is Running
      When I press "F8"
      Then the Debug adapter was sent no "next" request

  Rule: Every debug action is a Space chord

    Scenario Outline: Each Space chord does what its F-key does
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And the editor pane has focus
      When I press "Space"
      And I press "<letter>"
      Then the Debug adapter was sent a "<request>" request

      Examples:
        | letter | request    |
        | n      | next       |
        | i      | stepIn     |
        | o      | stepOut    |
        | c      | continue   |
        | q      | disconnect |

    Scenario: Space then r restarts the last session
      Given a Debug session was started from the Launch configuration "server"
      And the Debug session has ended
      When I press "Space"
      And I press "r"
      Then the Debug adapter for "rust" is asked for

    Scenario Outline: The other chords reach the rest of the Debug session
      Given a Debug session is Paused at "src/main.rs" line 3
      And the editor pane has focus
      When I press "Space"
      And I press "<letter>"
      Then <outcome>

      Examples:
        | letter | outcome                         |
        | e      | the Evaluator is open           |
        | s      | the Strip shows the Shell group |
        | h      | the Program output is hidden    |

    Scenario: Space chords work in the Variables
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And the Variables have focus
      When I press "Space"
      And I press "n"
      Then the Debug adapter was sent a "next" request for thread 1

    Scenario: Space in the Program output reaches the program
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Program output has focus
      When I press "Space"
      Then no Chord hint is shown
      And the Program output received the key "Space"

    Scenario: Space while inserting is a space
      Given a Debug session is Paused at "src/main.rs" line 3
      And I press "i" in the editor
      When I press "Space"
      Then no Chord hint is shown
      And the buffer has unsaved edits

  Rule: A tapped Space opens the Chord hint at once, and Escape cancels it

    Scenario: Tapping Space opens the Chord hint
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Space"
      Then the Chord hint is shown

    Scenario: The Chord hint lists every second key
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Space"
      Then the Chord hint lists the keys:
        | n |
        | i |
        | o |
        | c |
        | q |
        | r |
        | b |
        | B |
        | x |
        | e |
        | a |
        | s |
        | h |

    Scenario: Clicking a Chord hint entry does what its key does
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And I press "Space"
      When I click the Chord hint entry for "n"
      Then the Debug adapter was sent a "next" request for thread 1
      And no Chord hint is shown

    Scenario: Escape cancels a waiting Space
      Given a Debug session is Paused at "src/main.rs" line 3
      And I press "Space"
      When I press "Escape"
      Then no Chord hint is shown
      And the Debug adapter was sent no "next" request
      And Stepping mode is off

    Scenario: The second key takes the Chord hint down
      Given a Debug session is Paused at "src/main.rs" line 3
      And I press "Space"
      When I press "n"
      Then no Chord hint is shown

    Scenario: With no session the Chord hint offers only what works without one
      Given no Debug session exists
      When I press "Space"
      Then the Chord hint lists the keys:
        | b |
        | B |
        | x |
        | r |

  Rule: A chord leaves Stepping mode on, and any other key leaves it

    Scenario: A chord turns Stepping mode on
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Space"
      And I press "n"
      Then Stepping mode is on

    Scenario: In Stepping mode a stepping key acts without Space
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And Stepping mode is on
      When I press "i"
      Then the Debug adapter was sent a "stepIn" request for thread 1
      And Stepping mode is on

    Scenario: Any other key leaves Stepping mode and does what it always does
      Given a Debug session is Paused at "src/main.rs" line 3
      And Stepping mode is on
      And the cursor is at line 3 column 1
      When I press "j"
      Then Stepping mode is off
      And the cursor is on line 4

    Scenario: A key that leaves Stepping mode is not swallowed
      Given a Debug session is Paused at "src/main.rs" line 3
      And Stepping mode is on
      When I press "/"
      Then Stepping mode is off
      And the in-file search is open

    Scenario: The Variables title says when Stepping mode is on
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Space"
      And I press "n"
      Then the Variables title says "stepping"

    Scenario: Stepping mode ends with the session
      Given a Debug session is Paused at "src/main.rs" line 3
      And Stepping mode is on
      When the Debug adapter sends the event "terminated"
      Then Stepping mode is off

  Rule: Every debug action is a Chip on the Variables' Transport

    Scenario: The Transport holds a Chip per debug action
      Given a Debug session is Paused at "src/main.rs" line 3
      Then the Transport's Chips are:
        | continue    |
        | step-over   |
        | step-into   |
        | step-out    |
        | stop        |
        | restart     |
        | ask-ai      |
        | next-thread |

    Scenario Outline: Clicking a Chip does what its key does
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      When I click the "<chip>" Chip
      Then the Debug adapter was sent a "<request>" request

      Examples:
        | chip      | request    |
        | continue  | continue   |
        | step-over | next       |
        | step-into | stepIn     |
        | step-out  | stepOut    |
        | stop      | disconnect |

    Scenario: Each Chip names its F-key and its Space chord
      Given a Debug session is Paused at "src/main.rs" line 3
      Then the "step-over" Chip names the keys:
        | F8 |
        | ␣n |

    Scenario: Continue and pause share one Chip that says what pressing it does
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter reports the program continued
      Then the Transport's first Chip is "pause"

    Scenario: The stepping Chips are dimmed while Running
      Given a Debug session is Running
      Then the "step-over" Chip is dimmed
      And the "step-into" Chip is dimmed
      And the "step-out" Chip is dimmed
      And the "stop" Chip is not dimmed

    Scenario: The last Chip used by a key stays lit
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "F8"
      Then the "step-over" Chip is lit

    Scenario: The last Chip used by a click stays lit
      Given a Debug session is Paused at "src/main.rs" line 3
      When I click the "step-into" Chip
      Then the "step-into" Chip is lit

    Scenario: Only the last Chip used is lit
      Given a Debug session is Paused at "src/main.rs" line 3
      And I press "F8"
      And the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "step"
      When I press "F7"
      Then the "step-into" Chip is lit
      And the "step-over" Chip is not lit

    Scenario: A lit Chip stays lit with no timer
      Given a Debug session is Paused at "src/main.rs" line 3
      And I press "F8"
      When 10 seconds pass
      Then the "step-over" Chip is lit

  Rule: Short of room, every Chip sheds its keys together

    Scenario: A wide Transport shows every Chip with its keys
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Variables are 120 columns wide
      Then every Chip shows its keys

    Scenario: A narrow Transport sheds every Chip's keys at once
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Variables are 40 columns wide
      Then no Chip shows its keys
      And every Chip is drawn whole

  Rule: The debug keys are in the cheatsheet, from the same list as the Chord hint

    Scenario: The cheatsheet lists the debug keys while a session exists
      Given a Debug session is Paused at "src/main.rs" line 3
      Then the cheatsheet lists "F8"
      And the cheatsheet lists "␣n"

    Scenario: The cheatsheet leaves the F-keys out with no session
      Given no Debug session exists
      Then the cheatsheet does not list "F8"

    Scenario: Every Chord hint entry is a cheatsheet row
      Given a Debug session is Paused at "src/main.rs" line 3
      When I press "Space"
      Then every key the Chord hint lists is in the cheatsheet
