Feature: Being Paused

  Every pause brings the Debug session forward: the Strip switches to the Debug group and the Corner to
  the Frames. Keyboard focus stays where it was — a pause is something the program did, and a
  program that stole the keyboard mid-keystroke would put the user's next key somewhere they did
  not aim it. What the user then chooses to show holds until the next pause, and ending the session
  gives both slots back what they held before it began.

  Several threads can be Paused at once. The one being inspected stays put when another pauses; the
  others are flagged in the Frames and counted on the Transport, so a request held open is never
  forgotten. Continue resumes only the thread being inspected.

  While the program runs, what the last pause showed stays on screen, dimmed, and the Variables'
  title says Running — nothing dimmed is mistaken for current.

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

  Rule: The Paused line is marked, and an exception pause says so first

    Scenario: A pause marks the Paused line
      Given a Debug session is Running
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 3 with reason "breakpoint"
      Then the Paused line is "src/main.rs" line 3
      And the gutter marks line 3 as the Paused line
      And line 3 is highlighted across the editor's full width

    Scenario: A pause in a file that is not open opens it
      Given a Debug session is Running
      When the Debug adapter sends the "stopped" event for thread 1 at "src/lib.rs" line 8 with reason "step"
      Then the current buffer is "src/lib.rs"
      And the Paused line is "src/lib.rs" line 8

    Scenario: An exception pause draws the Paused line as an error
      Given a Debug session is Running
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "exception" and the text "attempt to add with overflow"
      Then the Paused line is drawn as "exception"
      And the first Variables row is the exception "attempt to add with overflow"

    Scenario: A pause for any other reason draws the Paused line plainly
      Given a Debug session is Running
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 3 with reason "breakpoint"
      Then the Paused line is drawn as "paused"

  Rule: Every pause brings the Debug group and the Frames forward, and moves no focus

    Scenario: A pause switches the Strip to the Debug group
      Given a Debug session is Running
      And the Strip shows the Shell group
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 3 with reason "breakpoint"
      Then the Strip shows the Debug group

    Scenario: A pause brings the Frames into the Corner
      Given the Corner shows the Buffers pane
      And a Debug session is Running
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 3 with reason "breakpoint"
      Then the Corner holds the Frames

    Scenario: A pause moves no keyboard focus
      Given a Debug session is Running
      And terminal 1 has focus
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 3 with reason "breakpoint"
      Then terminal 1 has the keyboard

    Scenario: A pause while typing leaves the next key where it was aimed
      Given a Debug session is Running
      And I press "i" in the editor
      And the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 3 with reason "breakpoint"
      When I type "x" in the editor
      Then the editor pane still has focus
      And the buffer has unsaved edits

    Scenario: What the user shows instead holds until the next pause
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Strip shows the Shell group
      When the Debug adapter reports the program continued
      Then the Strip shows the Shell group

    Scenario: The next pause brings them forward again
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Strip shows the Shell group
      And the Corner shows the Buffers pane
      And the Debug adapter reports the program continued
      When the Debug adapter sends the "stopped" event for thread 1 at "src/main.rs" line 4 with reason "step"
      Then the Strip shows the Debug group
      And the Corner holds the Frames

  Rule: Ending the session gives both slots back what they held before it began

    Scenario: The Strip and the Corner are restored when the session ends
      Given the Corner shows the Buffers pane
      And the Strip shows the Shell group
      And a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter sends the event "terminated"
      Then the Corner shows the Buffers pane
      And the Strip shows the Shell group

    Scenario: An empty Corner is empty again when the session ends
      Given the corner is empty
      And a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter sends the event "terminated"
      Then the corner is empty

    Scenario: The Debug group tab is gone with the session
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter sends the event "terminated"
      Then the Group tabs are:
        | Shells |

  Rule: Frames are grouped by thread, with runs of Library frames folded

    Scenario: Frames are listed under their thread
      Given a Debug session is Running
      And the Debug adapter's stack for thread 1 "main" is:
        | name   | file        | line |
        | add    | src/lib.rs  | 8    |
        | main   | src/main.rs | 4    |
      When the Debug adapter sends the "stopped" event for thread 1 at "src/lib.rs" line 8 with reason "breakpoint"
      Then the Frames are:
        | thread | frame |
        | main   | add   |
        | main   | main  |

    Scenario: A run of Library frames folds into one row that counts them
      Given a Debug session is Running
      And the Debug adapter's stack for thread 1 "main" is:
        | name      | file                                             | line |
        | handle    | src/lib.rs                                       | 8    |
        | call_once | /home/me/.rustup/toolchains/core/ops/function.rs | 250  |
        | poll      | /home/me/.cargo/registry/tokio/src/task.rs       | 90   |
        | run       | /home/me/.cargo/registry/tokio/src/runtime.rs    | 40   |
        | main      | src/main.rs                                      | 4    |
      When the Debug adapter sends the "stopped" event for thread 1 at "src/lib.rs" line 8 with reason "breakpoint"
      Then the Frames rows are:
        | kind    | shows  |
        | frame   | handle |
        | library | 3      |
        | frame   | main   |
      And the folded row is drawn dimmed

    Scenario: A Frame the adapter marks as not worth showing is a Library frame
      Given a Debug session is Running
      And the Debug adapter's stack for thread 1 "main" is:
        | name    | file        | line | hint        |
        | handle  | src/lib.rs  | 8    |             |
        | shim    | src/shim.rs | 2    | deemphasize |
        | main    | src/main.rs | 4    |             |
      When the Debug adapter sends the "stopped" event for thread 1 at "src/lib.rs" line 8 with reason "breakpoint"
      Then the Frames rows are:
        | kind    | shows  |
        | frame   | handle |
        | library | 1      |
        | frame   | main   |

    Scenario: A folded row unfolds on request
      Given a Debug session is Paused with 3 Library frames folded between "handle" and "main"
      And the Frames have focus
      And the Frames selection is the folded row
      When I press "Enter"
      Then the Frames rows are:
        | kind  | shows     |
        | frame | handle    |
        | frame | call_once |
        | frame | poll      |
        | frame | run       |
        | frame | main      |

  Rule: Choosing a Frame moves the whole inspection to that call

    Scenario: Choosing a Frame moves the Paused line to its call
      Given a Debug session is Paused in "add" at "src/lib.rs" line 8 called from "main" at "src/main.rs" line 4
      When I choose the Frame "main"
      Then the Paused line is "src/main.rs" line 4

    Scenario: Choosing a Frame asks for that Frame's Variables
      Given a Debug session is Paused in "add" at "src/lib.rs" line 8 called from "main" at "src/main.rs" line 4
      When I choose the Frame "main"
      Then the Debug adapter was sent a "scopes" request for the Frame "main"

    Scenario: Choosing a Frame points the Evaluator at it
      Given a Debug session is Paused in "add" at "src/lib.rs" line 8 called from "main" at "src/main.rs" line 4
      And the Evaluator is open holding "total"
      And I choose the Frame "main"
      When I run the Snippet
      Then the Debug adapter was sent an "evaluate" request in the Frame "main"

  Rule: A second thread pausing leaves the view on the thread being inspected

    Scenario: Another thread pausing does not move the Paused line
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      When the Debug adapter sends the "stopped" event for thread 2 at "src/lib.rs" line 8 with reason "breakpoint"
      Then the Paused line is "src/main.rs" line 3
      And the current buffer is "src/main.rs"

    Scenario: Other Paused threads are flagged in the Frames
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      When the Debug adapter sends the "stopped" event for thread 2 at "src/lib.rs" line 8 with reason "breakpoint"
      Then the Frames flag thread 2 as paused

    Scenario: Other Paused threads are counted on the Transport
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      When the Debug adapter sends the "stopped" event for thread 2 at "src/lib.rs" line 8 with reason "breakpoint"
      Then the "next-thread" Chip counts 1

    Scenario: The next-thread Chip jumps to the next Paused thread
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And the Debug adapter sends the "stopped" event for thread 2 at "src/lib.rs" line 8 with reason "breakpoint"
      When I click the "next-thread" Chip
      Then the Paused line is "src/lib.rs" line 8

    Scenario: With no other Paused thread the next-thread Chip is dimmed
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      Then the "next-thread" Chip is dimmed

    Scenario: Continue resumes only the thread being inspected
      Given a Debug session is Paused at "src/main.rs" line 3 on thread 1
      And the Debug adapter sends the "stopped" event for thread 2 at "src/lib.rs" line 8 with reason "breakpoint"
      When I press "F9"
      Then the Debug adapter was sent a "continue" request for thread 1 alone
      And thread 2 is still Paused

  Rule: While Running, the last pause stays on screen, dimmed, and the title says so

    Scenario: While Running the last Variables stay, dimmed
      Given a Debug session is Paused at "src/main.rs" line 3
      And the Variables show "count"
      When the Debug adapter reports the program continued
      Then the Variables show "count"
      And the Variables are drawn dimmed

    Scenario: While Running the last Frames stay, dimmed
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter reports the program continued
      Then the Frames are drawn dimmed

    Scenario: The Variables title says the program is Running
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter reports the program continued
      Then the Variables title says "running"

    Scenario: No Paused line is marked while Running
      Given a Debug session is Paused at "src/main.rs" line 3
      When the Debug adapter reports the program continued
      Then no Paused line is marked
