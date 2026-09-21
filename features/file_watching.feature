Feature: The TUI follows what happens on disk

  Varde is used with an AI writing files in the right pane, so the disk changes constantly
  and without the user's involvement. The tree, the review list and the diff on screen follow
  those changes immediately — a diff the AI has just made stale is the one thing a reviewer
  must never be shown.

  The editor is the exception, and only when it has something to lose: a buffer with unsaved
  edits is never overwritten by a change on disk. It is flagged instead, and reloading is the
  user's decision. A buffer with no unsaved edits simply follows the file.

  Background:
    Given the workspace root is "/home/me/projects/varde"

  Scenario: A file created in an expanded folder appears in the tree
    Given the folder "src" is expanded
    When "src/generated.js" is created on disk
    Then the file tree shows "src/generated.js"

  Scenario: A folder created in an expanded folder is listed as a folder
    Given the folder "src" is expanded
    When the folder "src/generated" is created on disk
    Then the file tree shows "src/generated" as a folder

  Scenario: A file created in a collapsed folder is not listed yet
    Given the folder "src" is collapsed
    When "src/generated.js" is created on disk
    Then the file tree does not show "src/generated.js"

  Scenario: A file created in a collapsed folder appears once it is expanded
    Given the folder "src" is collapsed
    And "src/generated.js" is created on disk
    When I expand "src"
    Then the file tree shows "src/generated.js"

  Scenario: A file appearing on disk never takes over the editor
    Given the folder "src" is expanded
    When the AI creates "/home/me/projects/varde/src/generated.js"
    Then no file was opened in the editor
    And the file tree shows "src/generated.js"

  Scenario: A branch checkout never takes over the editor
    When 40 files appear on disk from a branch checkout
    Then no file was opened in the editor

  Scenario: A file deleted on disk disappears from the tree
    Given the file tree shows "src/old.js"
    When "src/old.js" is deleted on disk
    Then the file tree does not show "src/old.js"

  Scenario: A clean buffer follows the file
    Given "src/tree.js" is open in the editor with no unsaved edits
    When "src/tree.js" is changed on disk
    Then the editor shows the version on disk
    And "src/tree.js" is not flagged as changed on disk

  Scenario: A buffer with unsaved edits is never overwritten
    Given "src/tree.js" is open in the editor with unsaved edits
    When "src/tree.js" is changed on disk
    Then the editor still shows the unsaved edits
    And "src/tree.js" is flagged as changed on disk

  Scenario: Reloading a flagged buffer takes the disk version
    Given "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    When I reload "src/tree.js"
    Then the editor shows the version on disk
    And "src/tree.js" is not flagged as changed on disk

  Scenario: The review list follows git while reviewing
    Given the project is a git repository
    And I open Review view
    And the review list shows:
      | src/tree.js |
    When "src/landing.js" is changed on disk
    Then the review list shows:
      | src/landing.js |
      | src/tree.js    |

  Scenario: The diff on screen follows the file it shows
    Given the diff for "src/tree.js" is shown
    When "src/tree.js" is changed on disk
    Then the diff for "src/tree.js" was re-read

  Scenario: A change to another file leaves the diff on screen alone
    Given the diff for "src/tree.js" is shown
    When "src/landing.js" is changed on disk
    Then no diff was re-read

  Scenario: Re-reading a diff keeps the reviewer where they were
    Given the diff for "src/tree.js" is shown
    And the editor pane has focus
    And I press "jj" in the editor
    When "src/tree.js" is changed on disk
    Then the diff cursor is on line 3

  Scenario: A file open in the editor is followed even with its folder collapsed
    Given "src/tree.js" is open in the editor with no unsaved edits
    And the folder "src" is collapsed
    Then "src/tree.js" is followed for changes

  Scenario: The file a diff is shown for is followed while reviewing
    Given the project is a git repository
    And I open Review view
    And the diff for "src/tree.js" is shown
    Then "src/tree.js" is followed for changes

  Scenario: A divergence is announced, not only marked
    Given "src/tree.js" is open in the editor with unsaved edits
    When "src/tree.js" is changed on disk
    Then the notice is "buffer-diverged"

  Scenario: A clean buffer following the file announces nothing
    Given "src/tree.js" is open in the editor with no unsaved edits
    When "src/tree.js" is changed on disk
    Then no notice was raised

  Scenario: A flagged buffer offers the three ways out
    Given "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    When I press "D" in the editor
    Then the modal is "diverged"

  Scenario: There is nothing to resolve on a buffer that agrees with disk
    Given "src/tree.js" is open in the editor with no unsaved edits
    When I press "D" in the editor
    Then the modal is "none"
    And the notice is "nothing-diverged"

  Scenario: D types a D while inserting
    Given "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    And I press "i" in the editor
    When I press "D" in the editor
    Then the modal is "none"

  Scenario: Resolving by reloading takes the disk version
    Given "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    And I press "D" in the editor
    When I resolve the divergence with "r"
    Then the editor shows the version on disk
    And "src/tree.js" is not flagged as changed on disk
    And the modal is "none"

  Scenario: Resolving by overwriting puts the unsaved edits on disk
    Given "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    And I press "D" in the editor
    When I resolve the divergence with "w"
    Then the unsaved edits were written to "src/tree.js"
    And "src/tree.js" is not flagged as changed on disk
    And the modal is "none"

  Scenario: Resolving by merging hands both versions to the AI
    Given an AI session is running in the AI pane
    And "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    And I press "D" in the editor
    When I resolve the divergence with "m"
    Then the AI pane was sent a prompt naming the file "src/tree.js"
    And the prompt carries both versions
    And the modal is "none"

  Scenario: A merge the AI has not done yet leaves the buffer flagged
    Given an AI session is running in the AI pane
    And "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    And I press "D" in the editor
    When I resolve the divergence with "m"
    Then "src/tree.js" is flagged as changed on disk
    And no file was written

  Scenario: Escape leaves a divergence standing
    Given "src/tree.js" is open in the editor with unsaved edits
    And "src/tree.js" is changed on disk
    And I press "D" in the editor
    When I press "Escape"
    Then the modal is "none"
    And "src/tree.js" is flagged as changed on disk
    And the editor still shows the unsaved edits
