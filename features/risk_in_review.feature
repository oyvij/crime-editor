Feature: Risk in Review view

  Review view shows what changed. It does not say whether the change raised the difficulty of
  maintaining those files, which is the question a reviewer most wants answered and least reliably
  answers by reading a diff. So entering Review view computes Risk for the files under review — the
  Scope is those files and never a mix — against their state at the revision the diff is already
  measured from, and the border shows the delta rather than the workspace's Risk count.

  A change that made things worse is marked as such: it is the signal that must not be missable.
  Leaving the view puts the workspace's own count back, because the number on the border always
  describes the Scope on screen.

  The Refactor loop can be run over exactly that Scope. It is the same machinery with a different file
  set, so every guarantee in `features/refactor_loop.feature` holds identically — a safety guarantee
  that depended on which Scope you chose would not be a guarantee. Arguably this is the more valuable
  of the two loops: it stops risk arriving rather than paying it down later.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the project is a git repository
    And the risk threshold is 20
    And the configured test command is "cargo test"
    And the iteration cap is 3
    And the working tree contains:
      | path        | git status |
      | src/keys.rs | modified   |
      | src/ui.rs   | modified   |
      | src/tree.rs | committed  |

  Scenario: Entering Review view measures the files under review, before and after
    When I open Review view
    Then an analysis was asked for over the scope "review"
    And the analysis covers exactly:
      | src/keys.rs |
      | src/ui.rs   |

  Scenario: The delta is measured from the revision the diff is measured from
    Given the review diff is measured from "aaaaaaaaaaaa"
    When I open Review view
    Then the review risk base revision is "aaaaaaaaaaaa"
    And the review risk base revision is the revision the review diff is measured from

  Scenario: The border shows the delta for the reviewed files, not the workspace's count
    Given I opened Review view
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 26         | 20        | 31             | 24            |
    Then the tree border shows the review delta
    And the review risk delta is -5

  Scenario: A change that made things worse is marked as such
    Given I opened Review view
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 38         | 30        | 31             | 24            |
    Then the review risk delta is 7
    And the review risk is marked worse

  Scenario: A change that lowered the figure is not marked worse
    Given I opened Review view
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 26         | 20        | 31             | 24            |
    Then the review risk is not marked worse

  Scenario: Leaving the view puts the workspace's count back
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
      | src/ui.rs   | draw     | 17   | 22         | 18        |
    And I opened Review view
    When I open Edit view
    Then the tree border shows the workspace risk count
    And the risk count is 2

  Scenario: The list names which part of the change added the risk
    Given the Risk list is shown
    And I opened Review view
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 38         | 30        | 31             | 24            |
      | src/ui.rs   | draw     | 17   | 19         | 15        | 22             | 18            |
    Then the Risk list shows:
      | function | figure | delta |
      | route    | 38     | 7     |
      | draw     | 19     | -3    |

  Scenario: No workspace Function reaches the Review-view list
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/tree.rs | icon     | 172  | 44         | 33        |
    And the Risk list is shown
    And I opened Review view
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 26         | 20        | 31             | 24            |
    Then the Risk list shows:
      | function | figure | delta |
      | route    | 26     | -5    |

  Scenario: A reviewed file in no language the analyser handles is not an improvement
    Given "docs/notes.txt" is modified
    And I opened Review view
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        | 31             | 24            |
    Then the review risk delta is 0
    And the review risk is not marked worse
    And "docs/notes.txt" contributes no figure

  Scenario: A Stale figure is recomputed for the reviewed files rather than shown as a delta
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    And the figure has gone stale
    When I open Review view
    Then an analysis was asked for over the scope "review"
    And the tree border risk state is "computing"

  Scenario: Review view offers a loop over exactly the files under review
    Given an AI session is running in the AI pane
    And the Risk list is shown
    And I opened Review view
    When I start the Refactor loop over the scope "review"
    Then the AI pane was sent a prompt naming the scope "review"
    And the AI pane was sent a prompt containing "src/keys.rs"
    And the AI pane was sent a prompt containing "src/ui.rs"
    And no prompt sent to the AI contains "src/tree.rs"

  Scenario: The loop edits nothing outside the Scope
    Given an AI session is running in the AI pane
    And I opened Review view
    And "src/tree.rs" had uncommitted changes before the loop started
    When Iteration 1 passes the Gate over the scope "review"
    Then the files the loop changed are:
      | src/keys.rs |
      | src/ui.rs   |
    And "src/tree.rs" is unchanged by the loop
    And nothing was committed

  Scenario: The Gate applies identically whichever Scope was chosen
    Given an AI session is running in the AI pane
    And I opened Review view
    And the Refactor loop is on Iteration 1 over the scope "review"
    And the Iteration touched:
      | src/keys.rs |
    When the tests finish failing:
      """
      test route::spells ... FAILED
      """
    Then Iteration 1 was restored from its snapshot
    And the Refactor loop stopped because "tests-failed"
    And nothing was committed

  Scenario: A failing Iteration reverts only the touched files within the Scope
    Given an AI session is running in the AI pane
    And I opened Review view
    And the Refactor loop is on Iteration 1 over the scope "review"
    And the Iteration touched:
      | src/keys.rs |
    When the tests finish failing:
      """
      test route::spells ... FAILED
      """
    Then the files restored are:
      | src/keys.rs |
    And "src/ui.rs" was not restored

  Scenario: Stopping a review-scoped loop is the same gesture in the same place
    Given an AI session is running in the AI pane
    And I opened Review view
    And Iteration 1 passed the Gate over the scope "review"
    And the Refactor loop is on Iteration 2 over the scope "review"
    When I stop the Refactor loop
    Then the Refactor loop is not running
    And Iteration 2 was restored from its snapshot
    And Iteration 1 was not restored from its snapshot
    And nothing was committed

  Scenario: A review-scoped loop is refused while any loop runs
    Given an AI session is running in the AI pane
    And the Refactor loop is on Iteration 1 over the scope "workspace"
    And I opened Review view
    When I start the Refactor loop over the scope "review"
    Then 1 Refactor loop is running
    And the Refactor loop refusal is "loop-already-running"

  Scenario: The cap stops a review-scoped loop the same way
    Given an AI session is running in the AI pane
    And the iteration cap is 2
    And I opened Review view
    And 1 Iteration has passed the Gate over the scope "review"
    And the Refactor loop is on Iteration 2 over the scope "review"
    And the tests finished passing
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 9          | 7         | 31             | 24            |
    Then Iteration 2 passed the Gate
    And the Refactor loop is not running
    And the Refactor loop stopped because "cap-reached"

  Scenario: After the loop the delta describes the new state of the reviewed files
    Given an AI session is running in the AI pane
    And I opened Review view
    And Iteration 1 passed the Gate over the scope "review"
    When the review figures arrive:
      | file        | function | line | cyclomatic | cognitive | was cyclomatic | was cognitive |
      | src/keys.rs | route    | 88   | 18         | 14        | 31             | 24            |
    Then the review risk delta is -13
    And the review risk is not marked worse
