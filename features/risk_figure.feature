Feature: Risk is computed for the workspace

  Opening Varde on a folder starts measuring Risk without being asked. The headline is the Risk
  count — how many Functions sit above the configured threshold — and it lives on the tree pane's top
  border: a spinner while the job runs, the count when it lands, so the same place on screen always
  answers the same question.

  The metric names what it measured. With no test coverage read the figure is CX, complexity alone,
  and it is labelled CX rather than CRAP because CRAP claims complexity weighted by test coverage and
  this figure is not that. A language the analyser does not handle produces an honest empty result
  rather than a fabricated zero, and a file it could not read is Unparsed — counted and shown rather
  than omitted in silence.

  A figure whose workspace has moved is a Stale figure: it says so, and it is never quietly recomputed
  while you type. Re-analysis happens when the commit moves, when it is asked for, and after an
  Iteration — never on a save (`docs/adr/0009-a-spinner-is-bounded-by-its-job.md`).

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And the project is a git repository
    And the risk threshold is 20

  Scenario: Opening a workspace starts measuring Risk without being asked
    When Varde opens "/home/me/projects/varde"
    Then an analysis was asked for over the scope "workspace"
    And the tree border risk state is "computing"

  Scenario: A job in flight is named, so a long analysis is not a hang
    Given an analysis is in flight over the scope "workspace"
    Then the tree border risk state is "computing"
    And the job in flight is "workspace-analysis"

  Scenario: The count replaces the spinner when the figures arrive
    Given an analysis is in flight over the scope "workspace"
    When the figures arrive for the scope "workspace":
      | file        | function   | line | cyclomatic | cognitive |
      | src/keys.rs | route      | 88   | 31         | 24        |
      | src/keys.rs | cheatsheet | 402  | 4          | 2         |
      | src/ui.rs   | draw       | 17   | 22         | 18        |
    Then the tree border risk state is "computed"
    And the risk count is 2
    And the risk metric is "CX"

  Scenario: With no test coverage read the figure is labelled CX, never CRAP
    Given no test coverage was read
    When the figures arrive for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    Then the risk metric is "CX"

  Scenario Outline: The count is Functions over the threshold, not an average or a percentage
    Given the risk threshold is <threshold>
    When the figures arrive for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
      | src/keys.rs | spell    | 210  | 12         | 9         |
      | src/ui.rs   | draw     | 17   | 22         | 18        |
    Then the risk count is <count>

    Examples:
      | threshold | count |
      | 10        | 3     |
      | 20        | 2     |
      | 30        | 1     |
      | 40        | 0     |

  Scenario: A workspace in no language the analyser handles is empty, never a clean bill of health
    Given every file in the workspace is in a language the analyser does not handle
    When the analysis finishes
    Then the tree border risk state is "nothing-analysed"
    And no risk count is shown

  Scenario: A file the analyser could not read is Unparsed, counted rather than dropped
    When the figures arrive for the scope "workspace" with 3 files Unparsed:
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    Then the risk count is 1
    And the unparsed count is 3

  Scenario: A space the analyser could not name is not a Function
    When the figures arrive for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
      | src/keys.rs |          | 140  | 26         | 20        |
    Then the risk count is 1
    And the unparsed count is 1

  Scenario: Analysis is never started by a keystroke
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    And "src/keys.rs" is open in the editor
    And the editor pane has focus
    When I type "hello"
    Then no analysis was asked for

  Scenario: Saving marks the figure stale and starts nothing
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    And "src/keys.rs" is open in the editor with unsaved edits
    When I run ":w"
    Then the tree border risk state is "stale"
    And no analysis was asked for

  Scenario: A stale figure is shown as stale, and still shows the figure it has
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
      | src/ui.rs   | draw     | 17   | 22         | 18        |
    When the figure goes stale
    Then the tree border risk state is "stale"
    And the risk count is 2

  Scenario: Reopening on the commit the figure was computed at runs no analysis
    Given the figures were recorded at the commit "aaaaaaaaaaaa"
    And "HEAD" resolves to "aaaaaaaaaaaa"
    When Varde opens "/home/me/projects/varde"
    Then the tree border risk state is "computed"
    And no analysis was asked for

  Scenario: Reopening after the commit has moved runs an analysis
    Given the figures were recorded at the commit "aaaaaaaaaaaa"
    And "HEAD" resolves to "bbbbbbbbbbbb"
    When Varde opens "/home/me/projects/varde"
    Then an analysis was asked for over the scope "workspace"

  Scenario: An explicit recompute runs whether or not the figure is stale
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    And the figure has not gone stale
    When I ask for the figures to be recomputed
    Then an analysis was asked for over the scope "workspace"

  Scenario: The recompute has a gesture of its own
    Given the figures were computed for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    And the figure has gone stale
    And the Risk list is shown
    When I click the "recompute-risk" action on the Risk list pane
    Then an analysis was asked for over the scope "workspace"

  Scenario: A recompute supersedes one already in flight rather than queueing
    Given an analysis is in flight over the scope "workspace"
    When I ask for the figures to be recomputed
    Then 1 analysis is in flight
    And the earlier analysis was superseded

  Scenario: The figures are written as Varde's own shape, carrying the commit and the metric
    Given "HEAD" resolves to "aaaaaaaaaaaa"
    When the figures arrive for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive | maintainability | lines |
      | src/keys.rs | route    | 88   | 31         | 24        | 41              | 96    |
    Then the figures were written to ".varde/risk.json"
    And the written figures record the commit "aaaaaaaaaaaa"
    And the written figures record the metric "CX"
    And the written figures list:
      | file        | function | line | cyclomatic | cognitive | maintainability | lines |
      | src/keys.rs | route    | 88   | 31         | 24        | 41              | 96    |

  Scenario: Computing the figures writes no git ignore rule
    Given the project has a ".gitignore"
    When the figures arrive for the scope "workspace":
      | file        | function | line | cyclomatic | cognitive |
      | src/keys.rs | route    | 88   | 31         | 24        |
    Then the project ".gitignore" is unchanged
