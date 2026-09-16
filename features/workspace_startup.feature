Feature: Opening CRIME on a folder

  CRIME opens on exactly one folder and that folder is the workspace. A path it cannot use
  as a workspace stops it before the TUI appears, and each way a path can be unusable gets
  its own explanation — a typo, a file, and a permissions problem are three different
  problems for the user to fix.

  An empty folder is not one of them. Starting CRIME in a fresh directory is how a project
  begins.

  Scenario: The folder opened becomes the workspace
    Given the folder "/home/me/projects/crime" exists
    When CRIME opens "/home/me/projects/crime"
    Then the workspace root is "/home/me/projects/crime"
    And the workspace title is "crime"

  Scenario: An empty folder is a valid workspace
    Given the folder "/home/me/projects/fresh" exists and is empty
    When CRIME opens "/home/me/projects/fresh"
    Then the workspace root is "/home/me/projects/fresh"
    And the file tree is empty

  Scenario: A path that does not exist stops CRIME
    Given "/home/me/projects/nope" does not exist
    When CRIME opens "/home/me/projects/nope"
    Then CRIME refuses to start
    And the reason is "no-such-folder"

  Scenario: A path that is a file stops CRIME
    Given "/home/me/projects/crime/README.md" is a file
    When CRIME opens "/home/me/projects/crime/README.md"
    Then CRIME refuses to start
    And the reason is "not-a-folder"

  Scenario: A folder that cannot be read stops CRIME
    Given the folder "/home/me/projects/locked" cannot be read
    When CRIME opens "/home/me/projects/locked"
    Then CRIME refuses to start
    And the reason is "folder-not-readable"
