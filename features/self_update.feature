Feature: Staying up to date

  CRIME is installed as a symlink on PATH pointing at the release binary inside its own
  checkout, so an ordinary release build is the install. The cost of that arrangement is
  drift: the checkout moves ahead, the binary keeps being the old one, and nothing says so.

  So CRIME learns two things about itself at launch — the Running version it was compiled
  from, and the Version its checkout claims — and offers an Update when the checkout is
  strictly ahead. Strictly: a checkout behind the binary is not an Update, because checking
  out an old branch must not nag anyone to downgrade.

  The check is one file read and a comparison. No network, no git, no watcher and no timer:
  the Running version cannot change while CRIME is running, and the checkout's Version only
  changes by something the user did themselves.

  A checkout counts as CRIME's own only when its manifest parses and names the crime
  package. A copied binary with nothing above it, another crate's manifest, and a manifest
  that is not valid TOML all leave no known checkout and offer nothing — which is what stops
  CRIME from ever building in a stranger's directory. That is the deliberate inverse of a
  config file, which the user handed to CRIME and which stops startup when it will not parse.

  Background:
    Given the workspace root is "/home/me/projects/blog"
    And CRIME's checkout is at "/home/me/src/crime"
    And the Running version is "0.1.0"

  Scenario: A checkout that has moved ahead offers an Update
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    When CRIME starts in the project
    Then an Update is available
    And CRIME's checkout is known to be "/home/me/src/crime"

  Scenario: A checkout at the same Version offers no Update
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.1.0"
      """
    When CRIME starts in the project
    Then no Update is available
    And CRIME's checkout is known to be "/home/me/src/crime"

  Scenario: A checkout behind the binary offers no Update
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.0.9"
      """
    When CRIME starts in the project
    Then no Update is available
    And CRIME's checkout is known to be "/home/me/src/crime"

  Scenario: A manifest for another crate is not CRIME's checkout
    Given the checkout manifest is:
      """
      [package]
      name = "ripgrep"
      version = "14.1.0"
      """
    When CRIME starts in the project
    Then no Update is available
    And CRIME's checkout is not known

  Scenario: A binary with no checkout above it offers no Update
    Given there is no checkout manifest
    When CRIME starts in the project
    Then no Update is available
    And CRIME's checkout is not known

  Scenario: A manifest that does not parse does not stop CRIME from starting
    Given the checkout manifest is:
      """
      [package
      name = "crime"
      version = "0.2.0"
      """
    When CRIME starts in the project
    Then CRIME started
    And no Update is available
    And CRIME's checkout is not known

  Scenario: An Update is not announced on the status line
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    When CRIME starts in the project
    Then no notice was raised

  Scenario: An Update outlasts the next notice
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    And CRIME started in the project
    And the review holds no comments
    When I submit the review
    Then the reviewer is told the review is empty
    And an Update is available

  # An Update is only a fact until something acts on it, and the thing that acts is an ordinary
  # release build in the terminal pane, where the compiler's output is already readable. It has to
  # name CRIME's checkout, because the pane's working directory is the workspace — a bare build
  # command would build whatever project happens to be open.
  #
  # `:update` is reachable from the palette too, under Help. It has to be: the key box used to be
  # where the command was advertised, and the box is about the view under it, so a command that
  # means the same thing everywhere is discoverable by the one gesture instead.

  Scenario: The build targets CRIME's checkout, not the open workspace
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    And CRIME started in the project
    When I ask CRIME to update from the command line
    Then the terminal has executed "cd /home/me/src/crime && cargo build --release"

  Scenario: The Update is reachable from the palette
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    And CRIME started in the project
    And the view palette is shown
    When I press "u"
    Then the terminal has executed "cd /home/me/src/crime && cargo build --release"
    And the view palette is not shown

  Scenario: A checkout path containing a space still builds
    Given CRIME's checkout is at "/home/me/my src/crime"
    And the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    And CRIME started in the project
    When I ask CRIME to update from the command line
    Then the terminal has executed "cd '/home/me/my src/crime' && cargo build --release"

  Scenario: With no known checkout the command notifies and runs nothing
    Given there is no checkout manifest
    And CRIME started in the project
    When I ask CRIME to update from the command line
    Then the user is told there is no known checkout
    And no command has been executed

  Scenario: The command is reachable when there is no Update to install
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.1.0"
      """
    And CRIME started in the project
    When I ask CRIME to update from the command line
    Then no Update is available
    And the terminal has executed "cd /home/me/src/crime && cargo build --release"

  Scenario: Updating touches nothing but the terminal
    Given the checkout manifest is:
      """
      [package]
      name = "crime"
      version = "0.2.0"
      """
    And CRIME started in the project
    When I ask CRIME to update from the command line
    Then no new AI session was started
    And no file was opened in the editor
    And no file was written
