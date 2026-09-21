Feature: Staying up to date

  Varde is installed as a symlink on PATH pointing at the release binary inside its own
  checkout, so an ordinary release build is the install. The cost of that arrangement is
  drift: the checkout moves ahead, the binary keeps being the old one, and nothing says so.

  So Varde learns two things about itself at launch — the Running version it was compiled
  from, and the Version its checkout claims — and offers an Update when the checkout is
  strictly ahead. Strictly: a checkout behind the binary is not an Update, because checking
  out an old branch must not nag anyone to downgrade.

  For a checkout install the check is one file read and a comparison. No network, no git, no
  watcher and no timer: the Running version cannot change while Varde is running, and the
  checkout's Version only changes by something the user did themselves.

  A binary install has no checkout to read, so it asks this repository once at startup for its
  latest Release and compares that Release's Version instead — one request, never a timer
  (docs/adr/0017-a-binary-install-updates-itself-from-a-release.md).

  A checkout counts as Varde's own only when its manifest parses and names the varde
  package. A copied binary with nothing above it, another crate's manifest, and a manifest
  that is not valid TOML all leave no known checkout and offer nothing — which is what stops
  Varde from ever building in a stranger's directory. That is the deliberate inverse of a
  config file, which the user handed to Varde and which stops startup when it will not parse.

  Background:
    Given the workspace root is "/home/me/projects/blog"
    And Varde's checkout is at "/home/me/src/varde"
    And the Running version is "0.1.0"

  Scenario: A checkout that has moved ahead offers an Update
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    When Varde starts in the project
    Then an Update to "0.2.0" is available
    And Varde's checkout is known to be "/home/me/src/varde"

  Scenario: A checkout at the same Version offers no Update
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.1.0"
      """
    When Varde starts in the project
    Then no Update is available
    And Varde's checkout is known to be "/home/me/src/varde"

  Scenario: A checkout behind the binary offers no Update
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.0.9"
      """
    When Varde starts in the project
    Then no Update is available
    And Varde's checkout is known to be "/home/me/src/varde"

  Scenario: A manifest for another crate is not Varde's checkout
    Given the checkout manifest is:
      """
      [package]
      name = "ripgrep"
      version = "14.1.0"
      """
    When Varde starts in the project
    Then no Update is available
    And Varde's checkout is not known

  Scenario: A binary with no checkout above it offers no Update
    Given there is no checkout manifest
    When Varde starts in the project
    Then no Update is available
    And Varde's checkout is not known

  Scenario: A manifest that does not parse does not stop Varde from starting
    Given the checkout manifest is:
      """
      [package
      name = "varde"
      version = "0.2.0"
      """
    When Varde starts in the project
    Then Varde started
    And no Update is available
    And Varde's checkout is not known

  Scenario: An Update is not announced on the status line
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    When Varde starts in the project
    Then no notice was raised

  Scenario: An Update outlasts the next notice
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    And Varde started in the project
    And the review holds no comments
    When I submit the review
    Then the reviewer is told the review is empty
    And an Update to "0.2.0" is available

  # An Update is only a fact until something acts on it, and the thing that acts is an ordinary
  # release build in the terminal pane, where the compiler's output is already readable. It has to
  # name Varde's checkout, because the pane's working directory is the workspace — a bare build
  # command would build whatever project happens to be open.
  #
  # `:update` is reachable from the palette too, under Help. It has to be: the key box used to be
  # where the command was advertised, and the box is about the view under it, so a command that
  # means the same thing everywhere is discoverable by the one gesture instead.

  Scenario: The build targets Varde's checkout, not the open workspace
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    And Varde started in the project
    When I ask Varde to update from the command line
    Then the terminal has executed "cd /home/me/src/varde && cargo build --release"

  Scenario: The Update is reachable from the palette
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    And Varde started in the project
    And the view palette is shown
    When I press "u"
    Then the terminal has executed "cd /home/me/src/varde && cargo build --release"
    And the view palette is not shown

  Scenario: A checkout path containing a space still builds
    Given Varde's checkout is at "/home/me/my src/varde"
    And the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    And Varde started in the project
    When I ask Varde to update from the command line
    Then the terminal has executed "cd '/home/me/my src/varde' && cargo build --release"

  Scenario: With neither a checkout nor a Release the command notifies and runs nothing
    Given there is no checkout manifest
    And Varde started in the project
    When I ask Varde to update from the command line
    Then the user is told there is nothing to update from
    And no command has been executed
    And Varde does not fetch a Release

  Scenario: The command is reachable when there is no Update to install
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.1.0"
      """
    And Varde started in the project
    When I ask Varde to update from the command line
    Then no Update is available
    And the terminal has executed "cd /home/me/src/varde && cargo build --release"

  Scenario: Updating touches nothing but the terminal
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.2.0"
      """
    And Varde started in the project
    When I ask Varde to update from the command line
    Then no new AI session was started
    And no file was opened in the editor
    And no file was written

  # A binary install learns of an Update from a Release rather than a manifest. The edge fetches
  # and hands the body back unread; which Version it names, whether that is newer, and which Asset
  # is this platform's are the core's to decide. Every way the answer can be useless — offline,
  # garbage, not newer, nothing built for this machine — is silent: a nag about the network is
  # worse than a marker a day late.

  Scenario: A binary install asks for the latest Release when it starts
    Given there is no checkout manifest
    When Varde starts in the project
    Then Varde asks for the latest Release

  Scenario: A checkout install never asks for a Release
    Given the checkout manifest is:
      """
      [package]
      name = "varde"
      version = "0.1.0"
      """
    When Varde starts in the project
    Then Varde does not ask for a Release

  Scenario: A newer Release offers an Update and is remembered for this platform
    Given there is no checkout manifest
    And Varde was built for "linux" on "x86_64"
    And Varde started in the project
    When the latest Release answers:
      """
      {"tag_name": "v0.2.0", "assets": [
        {"name": "varde-macos-aarch64", "browser_download_url": "https://example.test/varde-macos-aarch64"},
        {"name": "varde-linux-x86_64", "browser_download_url": "https://example.test/varde-linux-x86_64"},
        {"name": "SHA256SUMS", "browser_download_url": "https://example.test/SHA256SUMS"}
      ]}
      """
    Then an Update to "0.2.0" is available
    And the remembered Release is "0.2.0" with the Asset "https://example.test/varde-linux-x86_64" and the checksums "https://example.test/SHA256SUMS"
    And no notice was raised

  Scenario Outline: A Release that is not newer offers no Update
    Given there is no checkout manifest
    And Varde was built for "linux" on "x86_64"
    And Varde started in the project
    When the latest Release answers:
      """
      {"tag_name": "<tag>", "assets": [
        {"name": "varde-linux-x86_64", "browser_download_url": "https://example.test/varde-linux-x86_64"},
        {"name": "SHA256SUMS", "browser_download_url": "https://example.test/SHA256SUMS"}
      ]}
      """
    Then no Update is available
    And no Release is remembered
    And no notice was raised

    Examples:
      | tag    |
      | v0.1.0 |
      | v0.0.9 |

  Scenario: An answer that does not parse offers no Update
    Given there is no checkout manifest
    And Varde started in the project
    When the latest Release answers:
      """
      <html>rate limited</html>
      """
    Then no Update is available
    And no Release is remembered
    And no notice was raised

  Scenario: A request that failed offers no Update
    Given there is no checkout manifest
    And Varde started in the project
    When the request for the latest Release fails
    Then no Update is available
    And no Release is remembered
    And no notice was raised

  Scenario: A Release with nothing built for this platform offers no Update
    Given there is no checkout manifest
    And Varde was built for "linux" on "aarch64"
    And Varde started in the project
    When the latest Release answers:
      """
      {"tag_name": "v0.2.0", "assets": [
        {"name": "varde-linux-x86_64", "browser_download_url": "https://example.test/varde-linux-x86_64"},
        {"name": "SHA256SUMS", "browser_download_url": "https://example.test/SHA256SUMS"}
      ]}
      """
    Then no Update is available
    And no Release is remembered
    And no notice was raised

  # A binary install has no build to run, so `:update` puts the Release's Asset where the running
  # binary is and relaunches onto it. Fetching, verifying and replacing are the edge's; what each
  # outcome means is the core's. Relaunching is leaving, so it is refused exactly as quitting is —
  # and the binary on disk stays replaced across that refusal, so saving and asking again costs a
  # save and not a second download.

  Scenario: A binary install fetches the Release rather than building
    Given there is no checkout manifest
    And Varde was built for "linux" on "x86_64"
    And Varde started in the project
    And a newer Release for this platform has been found
    When I ask Varde to update from the command line
    Then Varde fetches the remembered Release
    And no command has been executed

  Scenario: The palette's Update fetches the Release on a binary install
    Given there is no checkout manifest
    And Varde was built for "linux" on "x86_64"
    And Varde started in the project
    And a newer Release for this platform has been found
    And the view palette is shown
    When I press "u"
    Then Varde fetches the remembered Release
    And no command has been executed

  Scenario Outline: A replacement that failed says which step and keeps the old binary
    Given there is no checkout manifest
    And Varde started in the project
    When replacing the binary fails at the <step> step
    Then the notice is "<notice>"
    And the binary is not known to be replaced
    And Varde does not relaunch

    Examples:
      | step     | notice          |
      | download | update-download |
      | no-asset | update-no-asset |
      | checksum | update-checksum |
      | replace  | update-replace  |

  Scenario: A replaced binary relaunches
    Given there is no checkout manifest
    And Varde started in the project
    When the binary has been replaced
    Then Varde relaunches
    And the project state was saved

  Scenario: A replaced binary with unsaved edits does not relaunch
    Given there is no checkout manifest
    And Varde started in the project
    And "notes.md" is open in the editor with unsaved edits
    When the binary has been replaced
    Then the reviewer is told there are unsaved changes
    And Varde does not relaunch
    And the binary is known to be replaced

  Scenario: Updating again after that refusal relaunches without fetching
    Given there is no checkout manifest
    And Varde started in the project
    And a newer Release for this platform has been found
    And "notes.md" is open in the editor with unsaved edits
    And the binary has been replaced
    And I write the buffer
    When I ask Varde to update from the command line
    Then Varde relaunches
    And Varde does not fetch a Release

  Scenario: Updating a binary install touches nothing but the binary
    Given there is no checkout manifest
    And Varde started in the project
    And a newer Release for this platform has been found
    When I ask Varde to update from the command line
    Then no new AI session was started
    And no file was opened in the editor
    And no file was written
    And no command has been executed
