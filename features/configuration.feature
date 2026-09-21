Feature: Configuration and state

  Varde reads TOML config from two places: ~/.varde/config.toml, seeded whenever it is
  missing, and <project>/.varde/config.toml, alongside the per-user state that lives in
  <project>/.varde/state.json.

  Project config overrides global config key by key — a project sets only what it needs to
  change and inherits the rest. A config file that does not parse stops Varde from starting,
  with an error precise enough to fix the file elsewhere.

  Varde never writes git ignore rules. What ends up committed is the user's decision.

  Background:
    Given the workspace root is "/home/me/projects/varde"

  Scenario: Starting in a project for the first time creates its config folder
    Given the project has no ".varde" folder
    When Varde starts in the project
    Then the project has a ".varde" folder

  # Seeded rather than left absent (Q38): nobody can configure a key they cannot
  # find, and `editor.tab_width` sat unnamed on every disk for its whole life.
  # Every key in the seeded file is commented out, which is what keeps "the
  # project sets nothing" true and keeps this binary's numbers from being frozen
  # into a file that outlives them. The third scenario below is that promise as
  # far as a scenario can carry it: the values quoted in the file are the
  # defaults, so uncommenting one would leave it green. What a live key breaks
  # is held by a unit test beside the constant instead.
  Scenario: Starting in a project for the first time seeds a config file
    Given the project has no ".varde" folder
    When Varde starts in the project
    Then the project ".varde/config.toml" was seeded

  Scenario: Starting in a project that already has a config file leaves it alone
    Given the project config is:
      """
      [editor]
      tab_width = 2
      """
    When Varde starts in the project
    Then the project ".varde/config.toml" is unchanged
    And the effective setting "editor.tab_width" is "2"

  # The global file is seeded by the same rule, from the template: every
  # Program row live, since a file is where Varde's programs are read from, and
  # every Setting commented out for the reason the project's are (ADR 0018).
  # That the template's Settings are the defaults is held by a unit test beside
  # it, for the reason given above.
  Scenario: Starting with no global config seeds it from the template
    Given there is no global config
    When Varde starts in the project
    Then the global config was seeded from the template

  Scenario: Starting with a global config leaves it alone
    Given the global config is:
      """
      [editor]
      tab_width = 2
      """
    When Varde starts in the project
    Then the global config is unchanged

  Scenario: A seeded config file changes nothing about the effective settings
    Given the global config is empty
    And the project has no ".varde" folder
    And Varde started in the project
    When Varde starts again with the config file it seeded
    Then the effective setting "editor.tab_width" is "4"
    And the effective setting "risk.threshold" is "15"

  Scenario: Starting again keeps the state already recorded
    Given the project ".varde/state.json" records the last view as "Review"
    When Varde starts in the project
    Then the current view is Review

  Scenario: Starting in a restored Story view reads the story sets
    Given a story set exists for the range "aaaaaaaaaaaa..bbbbbbbbbbbb"
    And the project ".varde/state.json" records the last view as "Story"
    When Varde starts in the project
    Then the story sets were read from ".varde/stories"
    And the story view state is "spine"

  Scenario: Starting in a restored Edit view reads no story sets
    Given a story set exists for the range "aaaaaaaaaaaa..bbbbbbbbbbbb"
    And the project ".varde/state.json" records the last view as "Edit"
    When Varde starts in the project
    Then no story sets were read

  Scenario: Starting in a restored Review view lands on the first changed file
    Given the working tree contains:
      | path        | git status |
      | src/tree.js | modified   |
    And the project ".varde/state.json" records the last view as "Review"
    When Varde starts in the project
    Then the tree selection is "src/tree.js"
    And the diff for "src/tree.js" is shown
    And the file tree pane has focus
    And an analysis was asked for over the scope "review"
    And no analysis was asked for over the scope "workspace"

  Scenario: Starting never touches the project's git files
    Given the project has a ".gitignore"
    And the project has no ".varde" folder
    When Varde starts in the project
    Then the project ".gitignore" is unchanged

  Scenario: A project setting overrides only the key it names
    Given the global config is:
      """
      [editor]
      tab_width = 4
      theme = "nord"
      """
    And the project config is:
      """
      [editor]
      tab_width = 2
      """
    When Varde starts in the project
    Then the effective setting "editor.tab_width" is "2"
    And the effective setting "editor.theme" is "nord"

  Scenario: Global settings apply when the project sets nothing
    Given the global config is:
      """
      [editor]
      theme = "nord"
      """
    And the project has no config file
    When Varde starts in the project
    Then the effective setting "editor.theme" is "nord"

  Scenario: A setting neither config names falls back to its default
    Given the global config is empty
    And the project has no config file
    When Varde starts in the project
    Then the effective setting "view.double_tap_ms" is "300"
    And the effective setting "editor.tab_width" is "4"

  Scenario: The Risk settings fall back to their defaults
    Given the global config is empty
    And the project has no config file
    When Varde starts in the project
    Then the effective setting "risk.threshold" is "15"
    And the effective setting "risk.max_iterations" is "10"

  Scenario: A malformed project config stops Varde from starting
    Given the project config is:
      """
      [editor
      tab_width = 2
      """
    When Varde starts in the project
    Then Varde refuses to start
    And the error names the file ".varde/config.toml"
    And the error names line 1
    And the fault is "not-toml"

  Scenario: A malformed global config stops Varde from starting
    Given the global config is:
      """
      [editor]
      tab_width = "two
      """
    When Varde starts in the project
    Then Varde refuses to start
    And the error names the file "~/.varde/config.toml"
    And the error names line 2
    And the fault is "not-toml"
