Feature: Tools — one list of everything CRIME runs

  The palette's `v` opens Tools: every program CRIME runs, grouped into language servers,
  formatters, requirements (`[facts.*]`) and speech — the synthesizer with its voice, and the
  player. One list rather than one per kind, because every row has the same shape: a name, the
  command it runs, and a status in one vocabulary shared by all of them.

  The list is what the config files name, and beside it every template row they do not name, read
  as `available`. That one rule answers a row the reader deleted and a row a newer CRIME added to
  its template: an upgrade never edits the file, and the new row still reaches the reader here
  (`docs/adr/0018-the-global-config-is-the-list-of-programs.md`).

  Whether a command is on this machine is the edge's to observe, so scenarios state it as a fact,
  the way the language server scenarios already do.

  Background:
    Given CRIME was built for "linux"

  Scenario: Tools groups its rows by kind, in one order
    Given there is no global config
    And the project has no config file
    When I open Tools
    Then the tools list is grouped as:
      | language-servers |
      | formatters       |
      | requirements     |
      | speech           |

  Scenario: A formatter row says whether its command is on this machine
    Given there is no global config
    And the project has no config file
    And the command "prettier" is on PATH
    And the command "black" is not on PATH
    And the command "pipx" is on PATH
    When I open Tools
    Then the formatter row for "markdown" is "installed"
    And the formatter row for "python" is "missing"

  Scenario: A template row the global file lacks is listed as available
    Given the global config is:
      """
      [lsp.rust]
      command = "rust-analyzer"
      extensions = ["rs"]
      """
    And the project has no config file
    And the command "rust-analyzer" is on PATH
    And the command "go" is on PATH
    And the command "npm" is on PATH
    And the command "uv" is on PATH
    When I open Tools
    Then the server row for "rust" is "installed"
    And the server row for "go" is "available"
    And the formatter row for "markdown" is "available"
    And the requirement row for "typescript_sdk" is "available"
    And the speech row for "synthesizer" is "available"

  Scenario: A row the file changed is shown as differing from its template
    Given the global config is:
      """
      [lsp.rust]
      command = "/opt/ra/rust-analyzer"
      extensions = ["rs"]

      [lsp.go]
      command = "gopls"
      extensions = ["go"]
      install.macos = "go install golang.org/x/tools/gopls@latest"
      install.linux = "go install golang.org/x/tools/gopls@latest"
      install.windows = "go install golang.org/x/tools/gopls@latest"
      """
    And the project has no config file
    When I open Tools
    Then the server row for "rust" differs from its template
    And the server row for "go" is the template's

  Rule: Taking a row is the whole install

    One key writes the row and runs what installs it. A row the global file lacks is appended from
    the template, with any `[facts.*]` row its values name, and nothing already in the file is
    touched. The install runs in the shell pane, visibly, where a `sudo` prompt can be answered, and
    reports its exit status through a sentinel file the watcher already sees — the way a clone does
    (`docs/adr/0015-a-clone-is-the-users-own-git.md`). A file CRIME cannot parse is refused before
    anything is written or run.

    Scenario: Taking an available row appends it to the global config and runs its install
      Given the global config is:
        """
        # Servers I chose myself.
        [lsp.rust]
        command = "rust-analyzer"
        extensions = ["rs"]
        """
      And the project has no config file
      And the command "gopls" is not on PATH
      And the command "go" is on PATH
      When I take the server row for "go"
      Then the global config still holds everything it held
      And the global config names the row "lsp.go"
      And the shell pane runs "go install golang.org/x/tools/gopls@latest" reporting its exit status

    Scenario: Taking a row whose values name a requirement appends the requirement too
      Given the global config is:
        """
        [lsp.rust]
        command = "rust-analyzer"
        extensions = ["rs"]
        """
      And the project has no config file
      And the command "npm" is on PATH
      When I take the server row for "vue"
      Then the global config names the row "lsp.vue"
      And the global config names the row "facts.typescript_sdk"

    Scenario: Taking a row the global config already has only runs its install
      Given the global config is:
        """
        [lsp.go]
        command = "gopls"
        extensions = ["go"]
        install.linux = "my-own-installer gopls"
        """
      And the project has no config file
      And the command "gopls" is not on PATH
      And the command "my-own-installer" is on PATH
      When I take the server row for "go"
      Then the global config is unchanged
      And the shell pane runs "my-own-installer gopls" reporting its exit status

    Scenario: A global config that no longer parses is refused, and nothing is written or run
      Given the global config is:
        """
        [lsp.rust]
        command = "rust-analyzer"
        extensions = ["rs"]
        """
      And the project has no config file
      And the command "go" is on PATH
      And the global config has since been edited to:
        """
        [lsp.rust
        command = "rust-analyzer"
        """
      When I take the server row for "go"
      Then the editor refuses with "broken-config"
      And the global config is unchanged
      And no command has been executed

    Scenario: An install that exits with a failure says so on its row
      Given the global config is:
        """
        [lsp.go]
        command = "gopls"
        extensions = ["go"]
        install.linux = "go install golang.org/x/tools/gopls@latest"
        """
      And the project has no config file
      And the command "gopls" is not on PATH
      And the command "go" is on PATH
      And I took the server row for "go"
      When the install reports the exit status "1"
      Then the server row for "go" is "install-failed"

  Rule: A missing package manager is refused before anything runs

    The program an install command starts with, or the one after `sudo`, is probed on `PATH` like
    any command. A row whose package manager is missing reads `needs-installer` and names it, and
    taking it writes nothing and runs nothing: package managers are `install.sh`'s to install.

    Scenario: A row whose install starts with a program not on PATH names it
      Given the global config is:
        """
        [lsp.go]
        command = "gopls"
        extensions = ["go"]
        install.linux = "go install golang.org/x/tools/gopls@latest"
        """
      And the project has no config file
      And the command "gopls" is not on PATH
      And the command "go" is not on PATH
      When I open Tools
      Then the server row for "go" is "needs-installer"
      And the server row for "go" needs the installer "go"

    Scenario: Taking a row whose package manager is missing writes nothing and runs nothing
      Given the global config is:
        """
        [lsp.rust]
        command = "rust-analyzer"
        extensions = ["rs"]
        """
      And the project has no config file
      And the command "gopls" is not on PATH
      And the command "go" is not on PATH
      When I take the server row for "go"
      Then the editor refuses with "needs-installer"
      And the global config is unchanged
      And no command has been executed

    Scenario: A row whose command is installed does not ask for its package manager
      Given the global config is:
        """
        [lsp.go]
        command = "gopls"
        extensions = ["go"]
        install.linux = "go install golang.org/x/tools/gopls@latest"
        """
      And the project has no config file
      And the command "gopls" is on PATH
      And the command "go" is not on PATH
      When I open Tools
      Then the server row for "go" is "installed"
