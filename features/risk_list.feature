Feature: The Risk list

  A pane listing the Functions worth fixing, worst first. It is toggled from the palette, sits
  beneath the tree at the tree's width, and takes its width from the shell pane — the mirror of what
  the AI pane's tall shape does from the other side. Hiding it gives the shell pane its full width
  back.

  It costs no new key binding. Focus is directional, so the pane is reachable by geometry — in every
  direction, both ways round: the shell pane's left edge is where this pane ends, so the leftward
  gesture reaches it back from there. A pane that could be left sideways and not re-entered was
  reachable only from the tree, two panes away. A palette row is discoverable because the palette
  lists it. Toggling it on moves focus into it, because opening a pane in order to use it should not
  need a second gesture.

  A row names a Function, the line it starts on and its figure — the line leading the figures, hard
  against the right-hand border, because a row is truncated from the right and where to go is the half
  that must survive a narrow pane. The selected row's file goes in the border, because the pane
  is narrower than a path and one ordering rule (worst first, across the whole Scope) is worth more
  than rows grouped under file headers. `Enter` or a click opens that file with the cursor on the
  Function — `Enter` following into the editor because it is the deliberate "take me there", a click
  leaving the keyboard in the pane it clicked, which is the same exception the tree's own click makes.
  The selection is a Row selection: it names something to go to rather than text you picked, so it is
  never copied and never becomes the selection — and a drag over the pane picks nothing at all,
  rather than characters off whatever pane lies behind it.

  The pane's own two actions live on its border, where the figure is, and are reached by stepping down
  off the end of the list — one slot past the last row, the way the Story spine's Remainder is — then
  along with `Left` and `Right`. `Enter` runs the one that is lit, through the same event its icon
  raises when it is clicked. An action reachable only by mouse is an action half the users do not
  have.

  Each row also carries an action asking an AI session for a suggested refactor of that one Function.
  One prompt, reviewed by you — no Iteration, no Gate, no revert, nothing committed. The full gated
  loop is `features/refactor_loop.feature`.

  Where the exact rectangles live is a layout unit test, not a scenario: this feature file is about
  what the pane holds and what the keys and the mouse do to it.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the project is a git repository
    And the risk threshold is 20
    And the figures were computed for the scope "workspace":
      | file        | function   | line | cyclomatic | cognitive |
      | src/keys.rs | route      | 88   | 31         | 24        |
      | src/ui.rs   | draw       | 17   | 22         | 18        |
      | src/keys.rs | cheatsheet | 402  | 4          | 2         |

  Scenario: The palette offers the pane in the group the panes live in
    Given the view palette is shown
    Then the view palette offers "Risk" in the group "Panes" under the key "k"

  Scenario: Picking the entry shows the pane and puts focus in it
    Given the Risk list is hidden
    And the view palette is shown
    When I click the palette entry "Risk"
    Then the Risk list is shown
    And the Risk list pane has focus

  Scenario: Picking it again hides the pane and returns focus to the tree
    Given the Risk list is shown
    And the view palette is shown
    When I click the palette entry "Risk"
    Then the Risk list is hidden
    And the file tree pane has focus

  Scenario: With the pane hidden the downward gesture from the tree still reaches the shell
    Given the Risk list is hidden
    And the file tree pane has focus
    When I press "Alt+j"
    Then the terminal pane has focus

  Scenario: With the pane shown the downward gesture from the tree reaches it
    Given the Risk list is shown
    And the file tree pane has focus
    When I press "Alt+j"
    Then the Risk list pane has focus

  Scenario: The shell pane is still reachable below the Risk list
    Given the Risk list is shown
    And the Risk list pane has focus
    When I press "Alt+j"
    Then the terminal pane has focus

  Scenario: The pane is reachable back from the shell, whose left edge is where it ends
    Given the Risk list is shown
    And the terminal pane has focus
    When I press "Alt+h"
    Then the Risk list pane has focus

  Scenario: With the pane hidden there is nothing to the shell's left
    Given the Risk list is hidden
    And the terminal pane has focus
    When I press "Alt+h"
    Then the terminal pane has focus

  Scenario: The pane's visibility is mine, and survives a restart
    Given the project ".crime/state.json" records the Risk list as shown
    When CRIME starts in the project
    Then the Risk list is shown

  Scenario: A pane never opened stays closed on a restart
    Given the project ".crime/state.json" records the Risk list as hidden
    When CRIME starts in the project
    Then the corner is empty

  Scenario: The list is a worklist, worst first
    Given the Risk list is shown
    Then the Risk list shows:
      | function | figure |
      | route    | 31     |
      | draw     | 22     |

  Scenario: Everything can be shown, for a figure that is not yet a problem
    Given the Risk list is shown
    When I show every Function in the Risk list
    Then the Risk list shows:
      | function   | figure |
      | route      | 31     |
      | draw       | 22     |
      | cheatsheet | 4      |

  Scenario: The pane says how much of the workspace the figure does not describe
    Given 3 files were Unparsed
    And the Risk list is shown
    Then the Risk list unparsed count is 3

  Scenario: Opening the pane during a job is not a blank stare
    Given no figures have been computed
    And an analysis is in flight over the scope "workspace"
    When I show the Risk list
    Then the Risk list state is "computing"
    And the Risk list is empty

  Scenario: A stale list is shown, marked stale, rather than no list at all
    Given the figure has gone stale
    And an analysis is in flight over the scope "workspace"
    When I show the Risk list
    Then the Risk list state is "stale"
    And the Risk list shows:
      | function | figure |
      | route    | 31     |
      | draw     | 22     |

  Scenario: The selection moves with the same unmodified motions the tree uses
    Given the Risk list is shown
    And the Risk list selection is "route"
    When I press "j"
    Then the Risk list selection is "draw"

  Scenario: The selection moves back up
    Given the Risk list is shown
    And the Risk list selection is "draw"
    When I press "k"
    Then the Risk list selection is "route"

  Scenario: The selected row's file is in the border, because the pane is narrower than a path
    Given the Risk list is shown
    And the Risk list selection is "route"
    When I press "j"
    Then the Risk list selection is "draw"
    And the Risk list border shows the file "src/ui.rs"

  Scenario: Enter opens the file with the cursor on the Function
    Given the Risk list is shown
    And the Risk list pane has focus
    And the Risk list selection is "draw"
    When I press "Enter"
    Then "/home/me/projects/crime/src/ui.rs" is open in the editor
    And the cursor is on line 17
    And the editor pane has focus

  Scenario: A click on a row opens what Enter opens and leaves the keyboard in the pane
    Given the Risk list is shown
    When I click the Risk list row "draw"
    Then "/home/me/projects/crime/src/ui.rs" is open in the editor
    And the cursor is on line 17
    And the Risk list pane has focus

  Scenario: Clicking a row is not the only way into the pane, so the next motion is the list's
    Given the Risk list is shown
    When I click the Risk list row "route"
    And I press "j"
    Then the Risk list selection is "draw"

  Scenario: A stale list still opens the file, and stays marked stale
    Given the figure has gone stale
    And the Risk list is shown
    When I click the Risk list row "draw"
    Then "/home/me/projects/crime/src/ui.rs" is open in the editor
    And the Risk list state is "stale"

  Scenario: A Row selection is not text, so it is never copied
    Given the Risk list is shown
    And the Risk list pane has focus
    And the Risk list selection is "route"
    And the terminal shows:
      """
      $ cargo test
      test result: ok. 1003 passed
      """
    When I drag from the corner pane's first row to its second
    And I press "Ctrl+c"
    Then the clipboard holds nothing
    And the selection holds nothing

  Scenario: The wheel scrolls without moving the selection
    Given the Risk list is shown
    And the Risk list holds 40 Functions
    And the Risk list selection is the first row
    When I scroll down 3 times with the pointer over the risk pane
    Then the Risk list first visible row is row 3
    And the Risk list selection is the first row

  Scenario: Every event but the wheel pulls the selection back into view
    Given the Risk list is shown
    And the Risk list holds 40 Functions
    And the Risk list selection is the first row
    And I scroll down with the pointer over the risk pane
    When I press "j"
    Then the Risk list selection is in view

  Scenario: A row offers an action asking for a refactor of that Function
    Given the Risk list is shown
    And the Risk list pane has focus
    And the Risk list selection is "route"
    Then the Risk list row actions offered are:
      | refactor-function |

  Scenario: The pane itself offers the recompute, where the figure is
    Given the Risk list is shown
    And the Risk list pane has focus
    Then the Risk list pane actions offered are:
      | recompute-risk       |
      | start-refactor-loop  |

  Scenario: Stepping down off the end of the list reaches the pane's own actions
    Given the Risk list is shown
    And the Risk list pane has focus
    And the Risk list selection is "draw"
    When I press "j"
    Then the keyboard is on the Risk list pane actions
    And the armed Risk list action is "recompute-risk"

  Scenario: Along the actions with Right, and no further than the last
    Given the Risk list is shown
    And the keyboard is on the Risk list pane actions
    When I press "Right"
    Then the armed Risk list action is "start-refactor-loop"

  Scenario: Enter runs the armed action, exactly as clicking its icon does
    Given the Risk list is shown
    And the keyboard is on the Risk list pane actions
    When I press "Enter"
    Then an analysis was asked for over the scope "workspace"

  Scenario: Up is the way back out, and it lets the icons go
    Given the Risk list is shown
    And the keyboard is on the Risk list pane actions
    When I press "k"
    Then the Risk list selection is "draw"
    And no Risk list action is armed

  Scenario: A list with nothing in it still reaches its recompute
    Given the risk threshold is 500
    And the Risk list is shown
    And the Risk list pane has focus
    Then the Risk list is empty
    And the keyboard is on the Risk list pane actions

  Scenario: The action's prompt carries what CRIME already measured
    Given the Risk list is shown
    And an AI session is running in the AI pane
    When I ask for a refactor of the Function "route"
    Then the AI pane was sent a prompt naming the Function "route"
    And the AI pane was sent a prompt naming the file "src/keys.rs"
    And the AI pane was sent a prompt naming the figure 31
    And the prompt was submitted to the AI

  Scenario: The row action's prompt asks for a meaningful split, not just a lower figure
    Given the Risk list is shown
    And an AI session is running in the AI pane
    When I ask for a refactor of the Function "route"
    Then the AI pane was sent a prompt asking for splits that stand on their own
    And the AI pane was sent a prompt naming structural scattering as a failed pass

  Scenario: Clicking the action icon does what the keyboard does
    Given the Risk list is shown
    And an AI session is running in the AI pane
    And the Risk list selection is "route"
    When I click the "refactor-function" action on the Risk list row "route"
    Then the AI pane was sent a prompt naming the Function "route"
    And the prompt was submitted to the AI

  Scenario: With no session running the action starts one rather than failing silently
    Given the Risk list is shown
    And no AI session is running in the AI pane
    And the effective setting "ai.command" is "claude"
    And I ask for a refactor of the Function "route"
    When the AI session is ready for input
    Then an AI session was started with "claude"
    And the AI pane was sent a prompt naming the Function "route"

  Scenario: The prompt is held until the session has spoken
    Given the Risk list is shown
    And no AI session is running in the AI pane
    When I ask for a refactor of the Function "route"
    Then no prompt was sent to the AI

  Scenario: A single Function's ask is one prompt and nothing else
    Given the Risk list is shown
    And an AI session is running in the AI pane
    When I ask for a refactor of the Function "route"
    Then exactly 1 prompt was sent to the AI
    And no Refactor loop is running
    And no test command was run
    And no snapshot was taken
    And nothing was committed
    And no command has been executed

  Scenario: Nothing in what was sent names a provider
    Given the Risk list is shown
    And an AI session is running in the AI pane
    When I ask for a refactor of the Function "route"
    Then the prompt names no AI provider
