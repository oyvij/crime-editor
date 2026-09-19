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
