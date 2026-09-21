Feature: Terminal splits

  The terminal strip holds one shell until it is split. `:split` puts a second
  shell beside the one that has the keyboard, started in that shell's own
  folder — a `cd` made in one split is where its neighbour begins — and the
  new one takes the keyboard. How many shells there are is the edge's fact: it
  starts them and watches them exit, and tells the core the count, which is
  why the keyboard only moves to a split the edge actually holds.

  A command Varde pushes at the terminal — a tree action's `touch`, an install
  line — goes to a shell whose prompt is waiting, never to one running a job,
  where it would be the job's input. The edge tells which is which. When every
  split is busy, a new shell is split off and the command waits for its prompt.

  Background:
    Given the workspace root is "/home/me/projects/varde"

  Scenario: Splitting asks for a shell beside the focused one
    Given the terminal holds 1 shell
    When I split the terminal from the command line
    Then a shell is asked for beside terminal 1

  Scenario: Splitting the middle one puts the new shell right after it
    Given the terminal holds 3 shells
    And terminal 2 has focus
    When I split the terminal from the command line
    Then a shell is asked for beside terminal 2

  Scenario: The new shell takes the keyboard once the edge holds it
    Given the terminal holds 1 shell
    And I split the terminal from the command line
    When the terminal holds 2 shells
    Then terminal 2 has the keyboard

  Scenario: A split that could not be started leaves the keyboard where it was
    Given the terminal holds 1 shell
    And I split the terminal from the command line
    When the terminal holds 1 shell
    Then terminal 1 has the keyboard

  Scenario: Focus moves right through the splits before it leaves the strip
    Given the terminal holds 3 shells
    And terminal 2 has focus
    When I press "Alt+l"
    Then terminal 3 has the keyboard

  Scenario: Focus moves left through the splits
    Given the terminal holds 3 shells
    And terminal 3 has focus
    When I press "Alt+h"
    Then terminal 2 has the keyboard

  Scenario: Focus right from the last split stays on it
    Given the terminal holds 2 shells
    And terminal 2 has focus
    When I press "Alt+l"
    Then terminal 2 has the keyboard

  Scenario: A split whose shell exited is gone and the keyboard falls back
    Given the terminal holds 3 shells
    And terminal 3 has focus
    When the terminal holds 2 shells
    Then terminal 2 has the keyboard

  Scenario: A pushed command goes to the focused split when its prompt is waiting
    Given the terminal holds 2 shells
    And terminal 2 has focus
    And the file tree shows the folder "src/tree"
    When I trigger "go here" on that folder
    Then the terminal input is "cd /home/me/projects/varde/src/tree"
    And terminal 2 has the keyboard

  Scenario: A pushed command avoids a split running a process
    Given the terminal holds 2 shells
    And terminal 1 is running a process
    And terminal 1 has focus
    And the file tree shows the folder "src/tree"
    When I trigger "go here" on that folder
    Then the terminal input is "cd /home/me/projects/varde/src/tree"
    And terminal 2 has the keyboard

  Scenario: With every split busy, a new shell is asked for and the command waits
    Given the terminal holds 1 shell
    And terminal 1 is running a process
    And the file tree shows the folder "src/tree"
    When I trigger "go here" on that folder
    Then a shell is asked for beside terminal 1
    And the terminal input is empty

  Scenario: The waiting command lands once the new shell prints its prompt
    Given the terminal holds 1 shell
    And terminal 1 is running a process
    And the file tree shows the folder "src/tree"
    And I trigger "go here" on that folder
    And the terminal holds 2 shells
    When terminal 2 prints its prompt
    Then the terminal input is "cd /home/me/projects/varde/src/tree"
    And terminal 2 has the keyboard

  Scenario: Another shell's prompt is not the one the command was waiting for
    Given the terminal holds 1 shell
    And terminal 1 is running a process
    And the file tree shows the folder "src/tree"
    And I trigger "go here" on that folder
    And the terminal holds 2 shells
    When terminal 1 prints its prompt
    Then the terminal input is empty
