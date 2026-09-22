Feature: Breakpoints

  A Breakpoint is core state, independent of any Debug session: it is set before a session starts,
  moves with its line as the Buffer is edited, and is remembered per project with the text its line
  held. What the Debug adapter says about one — bound, not bound and why, bound to another line — is
  kept for the session and never overwrites the line the user set.

  The gutter grows a Breakpoint column, leftmost, and a click there is the JetBrains gesture. Every
  Breakpoint action is a key too, and every one of them a Chip on the Breakpoint list's Transport or
  on its focused row, as `docs/adr/0022-every-action-has-a-chip.md` requires.

  Conditions, hit counts and log messages are the program's own language, handed to the Debug
  adapter as written and never read by Varde.

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

  Rule: A Breakpoint is set and removed by a click in its column, or by a key

    Scenario: Clicking the Breakpoint column sets a Breakpoint
      When I click the Breakpoint column on line 3
      Then "src/main.rs" line 3 has a Breakpoint

    Scenario: Clicking a Breakpoint removes it
      Given a Breakpoint on "src/main.rs" line 3
      When I click the Breakpoint column on line 3
      Then "src/main.rs" line 3 has no Breakpoint

    Scenario: Clicking the line numbers sets nothing
      When I click the line number of line 3
      Then "src/main.rs" line 3 has no Breakpoint

    Scenario: Space then b toggles a Breakpoint on the cursor's line
      Given the cursor is at line 3 column 1
      When I press "Space"
      And I press "b"
      Then "src/main.rs" line 3 has a Breakpoint

    Scenario: F-key toggles a Breakpoint while a session exists
      Given a Debug session is Running
      And the cursor is at line 2 column 1
      When I press "Ctrl+F8"
      Then "src/main.rs" line 2 has a Breakpoint

    Scenario: A Breakpoint set while no session exists is sent when one starts
      Given a Breakpoint on "src/main.rs" line 3
      And a Debug session was started from the Launch configuration "server"
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent a "setBreakpoints" request for "src/main.rs" line 3

    Scenario: A Breakpoint set mid-session is sent at once
      Given a Debug session is Running
      When I click the Breakpoint column on line 4
      Then the Debug adapter was sent a "setBreakpoints" request for "src/main.rs" line 4

    Scenario: Removing the last Breakpoint in a file sends that file an empty list
      Given a Breakpoint on "src/main.rs" line 3
      And a Debug session is Running
      When I click the Breakpoint column on line 3
      Then the Debug adapter was sent a "setBreakpoints" request for "src/main.rs" with no lines

  Rule: A Breakpoint moves with its line as the Buffer is edited

    Scenario: Inserting a line above a Breakpoint carries it down
      Given a Breakpoint on "src/main.rs" line 3
      And the cursor is at line 2 column 1
      When I press "O" in the editor
      Then "src/main.rs" line 4 has a Breakpoint
      And "src/main.rs" line 3 has no Breakpoint

    Scenario: Deleting a line above a Breakpoint carries it up
      Given a Breakpoint on "src/main.rs" line 3
      And the cursor is at line 2 column 1
      When I type "dd" in the editor
      Then "src/main.rs" line 2 has a Breakpoint

    Scenario: Deleting a Breakpoint's own line removes it
      Given a Breakpoint on "src/main.rs" line 3
      And the cursor is at line 3 column 1
      When I type "dd" in the editor
      Then "src/main.rs" has no Breakpoints

  Rule: Breakpoints are remembered per project, and a Bare workspace forgets them at exit

    Scenario: A Breakpoint is remembered in the project's state
      When I click the Breakpoint column on line 3
      Then the project ".varde/state.json" records a Breakpoint on "src/main.rs" line 3 holding "let count = 3;"

    Scenario: A remembered Breakpoint is back after a restart
      Given the project ".varde/state.json" records a Breakpoint on "src/main.rs" line 3 holding "let count = 3;"
      When Varde starts in the project
      Then "src/main.rs" line 3 has a Breakpoint

    Scenario: A Bare workspace keeps Breakpoints in its Sidecar
      Given the workspace is a Bare workspace
      When I click the Breakpoint column on line 3
      Then no file was written into the folder Varde was started in
      And "src/main.rs" line 3 has a Breakpoint

    Scenario: A Bare workspace's Breakpoints are gone when Varde starts again
      Given the workspace is a Bare workspace
      And I click the Breakpoint column on line 3
      When Varde starts again in the same folder
      Then "src/main.rs" has no Breakpoints

    Scenario: A remembered Breakpoint whose line no longer holds its text is Stale
      Given the project ".varde/state.json" records a Breakpoint on "src/main.rs" line 3 holding "let limit = 9;"
      When Varde starts in the project
      Then the Breakpoint list marks "src/main.rs" line 3 as "stale"

    Scenario: A Stale breakpoint is never moved onto another line
      Given the project ".varde/state.json" records a Breakpoint on "src/main.rs" line 3 holding "let limit = 9;"
      When Varde starts in the project
      Then "src/main.rs" line 2 has no Breakpoint
      And "src/main.rs" line 4 has no Breakpoint

    Scenario: A Stale breakpoint is not sent to the Debug adapter
      Given the project ".varde/state.json" records a Breakpoint on "src/main.rs" line 3 holding "let limit = 9;"
      And Varde starts in the project
      And a Debug session was started from the Launch configuration "server"
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent a "setBreakpoints" request for "src/main.rs" with no lines

  Rule: What the Debug adapter says about a Breakpoint is kept for the session

    Scenario: A Breakpoint the adapter could not bind is Unverified with its reason
      Given a Breakpoint on "src/main.rs" line 3
      And a Debug session is Running
      When the Debug adapter answers "setBreakpoints" for line 3 with verified false and message "no code at this line"
      Then the gutter draws line 3's Breakpoint as "unverified"
      And line 3's Breakpoint explains "no code at this line" on hover

    Scenario: A Breakpoint the adapter bound elsewhere is drawn on that line for the session
      Given a Breakpoint on "src/main.rs" line 3
      And a Debug session is Running
      When the Debug adapter answers "setBreakpoints" for line 3 with verified true at line 4
      Then the gutter draws a Breakpoint on line 4
      And the Breakpoint list lists "src/main.rs" line 3

    Scenario: A moved Breakpoint returns to the line it was set on when the session ends
      Given a Breakpoint on "src/main.rs" line 3
      And a Debug session is Running
      And the Debug adapter answers "setBreakpoints" for line 3 with verified true at line 4
      When the Debug adapter sends the event "terminated"
      Then the gutter draws a Breakpoint on line 3
      And the gutter draws no Breakpoint on line 4

    Scenario: The adapter's answer never overwrites the remembered line
      Given a Breakpoint on "src/main.rs" line 3
      And a Debug session is Running
      When the Debug adapter answers "setBreakpoints" for line 3 with verified true at line 4
      Then the project ".varde/state.json" records a Breakpoint on "src/main.rs" line 3 holding "let count = 3;"

  Rule: A Breakpoint can carry a condition, a hit count, a log message and a suspend scope

    Scenario: A condition is handed to the adapter as written
      Given a Breakpoint on "src/main.rs" line 3 with the condition "total > 10 && count != 0"
      And a Debug session was started from the Launch configuration "server"
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent a "setBreakpoints" request for line 3 with the condition "total > 10 && count != 0"

    Scenario: A hit count is handed to the adapter
      Given a Breakpoint on "src/main.rs" line 3 with the hit count "10"
      And a Debug session was started from the Launch configuration "server"
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent a "setBreakpoints" request for line 3 with the hit condition "10"

    Scenario: A Logpoint's message is handed to the adapter
      Given a Logpoint on "src/main.rs" line 3 with the message "count is {count}"
      And a Debug session was started from the Launch configuration "server"
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent a "setBreakpoints" request for line 3 with the log message "count is {count}"

    Scenario Outline: Each kind of Breakpoint has its own gutter glyph
      Given <breakpoint>
      Then the gutter draws line 3's Breakpoint as "<kind>"

      Examples:
        | breakpoint                                                         | kind        |
        | a Breakpoint on "src/main.rs" line 3                               | plain       |
        | a Breakpoint on "src/main.rs" line 3 with the condition "count > 1" | conditional |
        | a Breakpoint on "src/main.rs" line 3 with the hit count "10"        | conditional |
        | a Logpoint on "src/main.rs" line 3 with the message "hi"            | logpoint    |

    Scenario: A Breakpoint pauses only its own thread by default
      Given a Breakpoint on "src/main.rs" line 3
      Then the Breakpoint on "src/main.rs" line 3 suspends "thread"

    Scenario: A Breakpoint can be switched to pause every thread
      Given a Breakpoint on "src/main.rs" line 3
      And the Breakpoint box is open for "src/main.rs" line 3
      When I switch the Breakpoint's suspend scope
      Then the Breakpoint on "src/main.rs" line 3 suspends "all"

  Rule: A Breakpoint's properties are edited in a small floating box

    Scenario: The edit Chip on a Breakpoint opens its box
      Given a Breakpoint on "src/main.rs" line 3
      When I click the "edit" Chip on line 3's Breakpoint
      Then the Breakpoint box is open for "src/main.rs" line 3

    Scenario: Space then B opens the box for the Breakpoint on the cursor's line
      Given a Breakpoint on "src/main.rs" line 3
      And the cursor is at line 3 column 1
      When I press "Space"
      And I press "B"
      Then the Breakpoint box is open for "src/main.rs" line 3

    Scenario: A condition written in the box is kept
      Given a Breakpoint on "src/main.rs" line 3
      And the Breakpoint box is open for "src/main.rs" line 3
      When I write the condition "count > 1" in the Breakpoint box
      And I confirm the Breakpoint box
      Then the gutter draws line 3's Breakpoint as "conditional"

    Scenario: Escape leaves the Breakpoint as it was
      Given a Breakpoint on "src/main.rs" line 3
      And the Breakpoint box is open for "src/main.rs" line 3
      And I write the condition "count > 1" in the Breakpoint box
      When I press "Escape"
      Then the gutter draws line 3's Breakpoint as "plain"

  Rule: The Breakpoint list is a Corner occupant, with or without a session

    Scenario: The Breakpoint list shows every Breakpoint in the workspace
      Given a Breakpoint on "src/main.rs" line 3
      And a Breakpoint on "src/lib.rs" line 8
      When the Corner shows the Breakpoint list
      Then the Breakpoint list rows are:
        | src/lib.rs  | 8 |
        | src/main.rs | 3 |

    Scenario: The Breakpoint list is there without a session
      Given no Debug session exists
      When the Corner shows the Breakpoint list
      Then the Corner holds the Breakpoint list

    Scenario: Enter on a row goes to its line
      Given a Breakpoint on "src/lib.rs" line 8
      And the Corner shows the Breakpoint list
      And the Breakpoint list has focus
      When I press "Enter"
      Then the current buffer is "src/lib.rs"
      And the cursor is on line 8

    Scenario: The row's Chip removes one Breakpoint
      Given a Breakpoint on "src/main.rs" line 3
      And a Breakpoint on "src/lib.rs" line 8
      And the Corner shows the Breakpoint list
      When I click the "remove" Chip on the row for "src/lib.rs" line 8
      Then "src/lib.rs" has no Breakpoints
      And "src/main.rs" line 3 has a Breakpoint

    Scenario: The Transport's Chip clears them all
      Given a Breakpoint on "src/main.rs" line 3
      And a Breakpoint on "src/lib.rs" line 8
      And the Corner shows the Breakpoint list
      When I click the "clear-all" Chip
      Then the workspace has no Breakpoints

  Rule: Exception filters are the adapter's, switched in the Breakpoint list and remembered

    Scenario: The switches are exactly the filters the adapter reports
      Given a Debug session is Running
      And the Debug adapter reported the Exception filters:
        | id         | label          |
        | rust_panic | Rust: on panic |
      When the Corner shows the Breakpoint list
      Then the Breakpoint list switches are:
        | rust_panic |

    Scenario: Switching a filter mid-session tells the adapter at once
      Given a Debug session is Running
      And the Debug adapter reported the Exception filters:
        | id       | label    |
        | caught   | Caught   |
        | uncaught | Uncaught |
      And the Corner shows the Breakpoint list
      When I switch the Exception filter "caught" on
      Then the Debug adapter was sent a "setExceptionBreakpoints" request with the filters:
        | caught |

    Scenario: Exception filter choices are remembered per project and adapter
      Given a Debug session is Running
      And the Debug adapter reported the Exception filters:
        | id       | label    |
        | uncaught | Uncaught |
      And the Corner shows the Breakpoint list
      When I switch the Exception filter "uncaught" on
      Then the project ".varde/state.json" records the Exception filter "uncaught" on for "rust"

    Scenario: The next session starts with the remembered filters on
      Given the project ".varde/state.json" records the Exception filter "uncaught" on for "rust"
      And Varde starts in the project
      And a Debug session was started from the Launch configuration "server"
      And the Debug adapter reported the Exception filters:
        | id       | label    |
        | uncaught | Uncaught |
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent a "setExceptionBreakpoints" request with the filters:
        | uncaught |

    Scenario: One named exception class is offered where the adapter supports it
      Given a Debug session is Running
      And the Debug adapter reported it supports exception options
      When I pause on the exception class "java.lang.IllegalStateException"
      Then the Debug adapter was sent a "setExceptionBreakpoints" request naming "java.lang.IllegalStateException"

    Scenario: Naming an exception class is not offered where the adapter cannot
      Given a Debug session is Running
      And the Debug adapter reported it does not support exception options
      When the Corner shows the Breakpoint list
      Then the "exception-class" Chip is dimmed
