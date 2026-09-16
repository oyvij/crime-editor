Feature: Configuration and state

  CRIME reads TOML config from two places: ~/.crime/config.toml, created when CRIME is
  installed, and <project>/.crime/config.toml, alongside the per-user state that lives in
  <project>/.crime/state.json.

  Project config overrides global config key by key — a project sets only what it needs to
  change and inherits the rest. A config file that does not parse stops CRIME from starting,
  with an error precise enough to fix the file elsewhere.

  CRIME never writes git ignore rules. What ends up committed is the user's decision.

  Background:
    Given the workspace root is "/home/me/projects/crime"

  Scenario: Starting in a project for the first time creates its config folder
    Given the project has no ".crime" folder
    When CRIME starts in the project
    Then the project has a ".crime" folder

  # Seeded rather than left absent (Q38): nobody can configure a key they cannot
  # find, and `editor.tab_width` sat unnamed on every disk for its whole life.
  # Every key in the seeded file is commented out, which is what keeps "the
  # project sets nothing" true and keeps this binary's numbers from being frozen
  # into a file that outlives them. The third scenario below is that promise as
  # far as a scenario can carry it: the values quoted in the file are the
  # defaults, so uncommenting one would leave it green. What a live key breaks
  # is held by a unit test beside the constant instead.
  Scenario: Starting in a project for the first time seeds a config file
    Given the project has no ".crime" folder
    When CRIME starts in the project
    Then the project ".crime/config.toml" was seeded

  Scenario: Starting in a project that already has a config file leaves it alone
    Given the project config is:
      """
      [editor]
      tab_width = 2
      """
    When CRIME starts in the project
    Then the project ".crime/config.toml" is unchanged
    And the effective setting "editor.tab_width" is "2"

  Scenario: A seeded config file changes nothing about the effective settings
    Given the global config is empty
    And the project has no ".crime" folder
    And CRIME started in the project
    When CRIME starts again with the config file it seeded
    Then the effective setting "editor.tab_width" is "4"
    And the effective setting "risk.threshold" is "15"

  Scenario: Starting again keeps the state already recorded
    Given the project ".crime/state.json" records the last view as "Review"
    When CRIME starts in the project
    Then the current view is Review

  Scenario: Starting never touches the project's git files
    Given the project has a ".gitignore"
    And the project has no ".crime" folder
    When CRIME starts in the project
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
    When CRIME starts in the project
    Then the effective setting "editor.tab_width" is "2"
    And the effective setting "editor.theme" is "nord"

  Scenario: Global settings apply when the project sets nothing
    Given the global config is:
      """
      [editor]
      theme = "nord"
      """
    And the project has no config file
    When CRIME starts in the project
    Then the effective setting "editor.theme" is "nord"

  Scenario: A setting neither config names falls back to its default
    Given the global config is empty
    And the project has no config file
    When CRIME starts in the project
    Then the effective setting "view.double_tap_ms" is "300"
    And the effective setting "editor.tab_width" is "4"

  Scenario: The Risk settings fall back to their defaults
    Given the global config is empty
    And the project has no config file
    When CRIME starts in the project
    Then the effective setting "risk.threshold" is "15"
    And the effective setting "risk.max_iterations" is "10"

  Scenario: A malformed project config stops CRIME from starting
    Given the project config is:
      """
      [editor
      tab_width = 2
      """
    When CRIME starts in the project
    Then CRIME refuses to start
    And the error names the file ".crime/config.toml"
    And the error names line 1
    And the fault is "not-toml"

  Scenario: A malformed global config stops CRIME from starting
    Given the global config is:
      """
      [editor]
      tab_width = "two
      """
    When CRIME starts in the project
    Then CRIME refuses to start
    And the error names the file "~/.crime/config.toml"
    And the error names line 2
    And the fault is "not-toml"
