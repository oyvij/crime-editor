Feature: Starting and ending a Debug session

  A Debug session is laid over Edit view, never a View of its own: what you do while a program is
  Paused is write code. It is started from a Run mark beside a `main` or a test, or from a named
  Launch configuration, and it ends when the program ends or the user stops it.

  Varde speaks the Debug Adapter Protocol to a Debug adapter named in configuration, never in Varde
  (`docs/adr/0021-a-debug-adapter-is-a-hosted-child-reached-three-ways.md`). Nothing here drives a
  real adapter: a scripted Debug adapter in the World answers requests with canned replies and sends
  the events a scenario names, the way the canned Language server answers today. What is asserted is
  which requests reached it and in what order, and what state the session is in — never a pty, a
  port or a screen.

  That the adapter is alive is told by the edge, never remembered by the core: a spawn that was
  asked for and never happened is not a session.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a Debug adapter for "rust" is configured
    And the project config is:
      """
      [launch.server]
      adapter = "rust"
      request = "launch"
      args = { program = "target/debug/server" }
      """

  Rule: A Run mark stands beside whatever the language's rows say can be started

    Scenario: A main function carries a Run mark
      Given "src/main.rs" is open in the editor holding:
        """
        fn main() {
            println!("hi");
        }
        """
      Then line 1 carries a Run mark

    Scenario: A test carries a Run mark and a helper does not
      Given "src/lib.rs" is open in the editor holding:
        """
        fn helper() {}

        #[test]
        fn adds() {}
        """
      Then line 4 carries a Run mark
      And line 1 carries no Run mark

    Scenario: A language with no Run mark row has no Run marks
      Given "notes.txt" is open in the editor holding:
        """
        fn main() {}
        """
      Then no line carries a Run mark

    Scenario: A language Varde ships no Run mark row for gets Run marks from a configuration row
      Given the global config is:
        """
        [run.pytest]
        extensions = ["py"]
        query = '((function_definition name: (identifier) @name) @run (#match? @name "^test_"))'
        run = "pytest ${file} -k ${name}"
        debug = { adapter = "python", request = "launch", args = { module = "pytest", args = ["${file}", "-k", "${name}"] } }
        """
      And "tests/test_sum.py" is open in the editor holding:
        """
        def test_adds():
            pass
        """
      Then line 1 carries a Run mark

  Rule: A Run mark offers Run or Debug, and Run never types into a running job

    Scenario: Clicking a Run mark offers Run and Debug
      Given "src/main.rs" is open in the editor holding:
        """
        fn main() {}
        """
      When I click the Run mark on line 1
      Then the Run mark offers:
        | run   |
        | debug |

    Scenario: Run runs the command in a shell whose prompt is waiting
      Given "src/lib.rs" is open in the editor holding:
        """
        #[test]
        fn adds() {}
        """
      And the terminal holds 2 shells
      And terminal 1 is running a process
      When I choose "run" on the Run mark on line 2
      Then the terminal has executed "cargo test adds -- --exact"
      And terminal 2 has the keyboard
      And no Debug session exists

    Scenario: Run with every shell busy splits off a shell and waits for its prompt
      Given "src/lib.rs" is open in the editor holding:
        """
        #[test]
        fn adds() {}
        """
      And the terminal holds 1 shell
      And terminal 1 is running a process
      When I choose "run" on the Run mark on line 2
      Then a shell is asked for beside terminal 1
      And no command has been executed

    Scenario: Debug starts a session for exactly the marked function
      Given "src/lib.rs" is open in the editor holding:
        """
        #[test]
        fn adds() {}

        #[test]
        fn subtracts() {}
        """
      When I choose "debug" on the Run mark on line 2
      Then a Debug adapter for "rust" is asked for
      And the Debug adapter's launch arguments name "adds"
      And the Debug adapter's launch arguments do not name "subtracts"

  Rule: Launch configurations are named, global or per project, and the project's wins

    Scenario: A global Launch configuration is offered in every project
      Given the global config is:
        """
        [launch.orders]
        adapter = "java"
        request = "attach"
        args = { hostName = "localhost", port = 5005 }
        """
      When I open the launch palette
      Then the launch palette offers "orders"

    Scenario: A project Launch configuration beats a global one of the same name
      Given the global config is:
        """
        [launch.orders]
        adapter = "java"
        request = "attach"
        args = { hostName = "localhost", port = 5005 }
        """
      And the project config is:
        """
        [launch.orders]
        adapter = "java"
        request = "attach"
        args = { hostName = "localhost", port = 5006 }
        """
      And a Debug adapter for "java" is configured
      And the Debug adapter for "java" is ready
      When I start the Launch configuration "orders" from the palette
      Then the Debug adapter's attach arguments are:
        """
        {"hostName":"localhost","port":5006}
        """

    Scenario: Launch arguments reach the adapter untouched
      Given the project config is:
        """
        [launch.server]
        adapter = "rust"
        request = "launch"
        args = { program = "target/debug/server", cwd = "${workspace}", sourceLanguages = ["rust"] }
        """
      And the Debug adapter for "rust" is ready
      When I start the Launch configuration "server" from the palette
      Then the Debug adapter's launch arguments are:
        """
        {"program":"target/debug/server","cwd":"${workspace}","sourceLanguages":["rust"]}
        """

    Scenario: The palette lists global and project Launch configurations together
      Given the global config is:
        """
        [launch.orders]
        adapter = "java"
        request = "attach"
        args = { port = 5005 }
        """
      And the project config is:
        """
        [launch.server]
        adapter = "rust"
        request = "launch"
        args = {}
        """
      When I open the launch palette
      Then the launch palette offers "orders"
      And the launch palette offers "server"

  Rule: Startup follows the protocol, and Breakpoints are sent before configurationDone

    Scenario: A session is started in the protocol's order
      Given a Breakpoint on "src/main.rs" line 3
      And the Debug adapter for "rust" is ready
      And I start the Launch configuration "server" from the palette
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent, in order:
        | initialize             |
        | launch                 |
        | setBreakpoints         |
        | setExceptionBreakpoints |
        | configurationDone      |

    Scenario: Nothing is sent after launch until the adapter says it is initialized
      Given a Breakpoint on "src/main.rs" line 3
      And the Debug adapter for "rust" is ready
      When I start the Launch configuration "server" from the palette
      Then the Debug adapter was sent no "setBreakpoints" request
      And the Debug adapter was sent no "configurationDone" request

    Scenario: A started session is Running once it is configured
      Given the Debug adapter for "rust" is ready
      And I start the Launch configuration "server" from the palette
      When the Debug adapter sends the event "initialized"
      Then the Debug session is "running"

  Rule: A key and a Chip rerun the last session

    Scenario: Restart starts the last session again
      Given a Debug session was started from the Launch configuration "server"
      And the Debug session has ended
      When I press "Ctrl+F5"
      Then the Debug adapter for "rust" is asked for
      And the Debug adapter's launch arguments are those of "server"

    Scenario: The restart Chip does what the key does
      Given a Debug session was started from the Launch configuration "server"
      And the Debug session has ended
      When I click the "restart" Chip
      Then the Debug adapter for "rust" is asked for

    Scenario: Restart with no earlier session is refused out loud
      When I press "Ctrl+F5"
      Then the editor refuses with "no-last-session"
      And no Debug adapter is asked for

  Rule: Attach sessions detach on stop, and go Waiting when their program goes away

    Background:
      Given the global config is:
        """
        [launch.orders]
        adapter = "java"
        request = "attach"
        args = { hostName = "localhost", port = 5005 }
        """
      And a Debug adapter for "java" is configured

    Scenario: Stopping an attach session detaches and leaves the program running
      Given a Debug session was started from the Launch configuration "orders"
      When I press "Ctrl+F2"
      Then the Debug adapter was sent a "disconnect" request with "terminateDebuggee" false

    Scenario: An attach session whose program goes away is Waiting
      Given a Debug session was started from the Launch configuration "orders"
      When the Debug adapter sends the event "terminated"
      Then the Debug session is "waiting"

    Scenario: A Waiting session attaches again when the port answers
      Given a Debug session was started from the Launch configuration "orders"
      And the Debug adapter sends the event "terminated"
      When the edge reports the port 5005 answers
      Then the Debug adapter was sent 2 "attach" requests

    Scenario: Breakpoints are sent again after every re-attach
      Given a Breakpoint on "src/main/java/Orders.java" line 12
      And a Debug session was started from the Launch configuration "orders"
      And the Debug adapter sends the event "terminated"
      And the edge reports the port 5005 answers
      When the Debug adapter sends the event "initialized"
      Then the Debug adapter was sent 2 "setBreakpoints" requests for "src/main/java/Orders.java"

    Scenario: The Transport shows a Waiting session as Waiting
      Given a Debug session was started from the Launch configuration "orders"
      And Stepping mode is on
      When the Debug adapter sends the event "terminated"
      Then the Transport says "waiting"
      And Stepping mode is off
      And the "step-over" Chip is dimmed
      And the "continue" Chip is dimmed

    Scenario: A Launch configuration can opt out of re-attaching
      Given the global config is:
        """
        [launch.orders]
        adapter = "java"
        request = "attach"
        reattach = false
        args = { hostName = "build-box", port = 5005 }
        """
      And a Debug session was started from the Launch configuration "orders"
      When the Debug adapter sends the event "terminated"
      Then no Debug session exists

    Scenario: A Waiting session is ended only by stopping it
      Given a Debug session was started from the Launch configuration "orders"
      And the Debug adapter sends the event "terminated"
      When I press "Ctrl+F2"
      Then no Debug session exists

    Scenario: A re-attach the program refuses is reported and goes on Waiting
      Given a Debug session was started from the Launch configuration "orders"
      And the Debug adapter sends the event "terminated"
      And the edge reports the port 5005 answers
      When the Debug adapter answers "attach" with the error "handshake failed"
      Then the editor refuses with "launch-failed"
      And the Debug session is "waiting"

  Rule: An adapter a language server hosts is loaded into it, and asked there for its port

    The third way in (ADR 0021): the row names a language server, a plugin loaded into it when it
    starts, and a command sent to it once a session begins. The server answers with the port its
    adapter listens on, and from there the session is any other.

    Background:
      Given the global config is:
        """
        [lsp.java]
        command = "jdtls"
        extensions = ["java"]

        [dap.java]
        server = "java"
        command = "vscode.java.startDebugSession"
        plugin = { bundles = ["/opt/java-debug/plugin.jar"] }

        [launch.orders]
        adapter = "java"
        request = "attach"
        args = { hostName = "localhost", port = 5005 }
        """
      And Varde started in the project

    Scenario: The plugin is loaded into the language server when it starts
      When I open "src/main/java/Orders.java"
      Then the initialize request for "java" carried initialization options:
        """
        {"bundles":["/opt/java-debug/plugin.jar"]}
        """

    Scenario: Starting a session sends the configured command to the language server
      Given a language server for "java" is ready
      And I open "src/main/java/Orders.java"
      When I start the Launch configuration "orders" from the palette
      Then the language server for "java" was sent "workspace/executeCommand" with:
        """
        {"command":"vscode.java.startDebugSession"}
        """
      And no Debug adapter was spawned

    Scenario: The adapter is reached on the port the language server answers with
      Given a language server for "java" is ready
      And the Debug adapter for "java" is ready
      And I open "src/main/java/Orders.java"
      And I start the Launch configuration "orders" from the palette
      When the language server for "java" replies to "workspace/executeCommand" with:
        """
        41234
        """
      Then the Debug adapter is reached on port 41234
      And the Debug adapter was sent a "initialize" request

    Scenario: A re-attach asks the language server for a port again
      Given a language server for "java" is ready
      And the Debug adapter for "java" is ready
      And I open "src/main/java/Orders.java"
      And I start the Launch configuration "orders" from the palette
      And the language server for "java" replies to "workspace/executeCommand" with:
        """
        41234
        """
      And the Debug adapter sends the event "terminated"
      When the edge reports the port 5005 answers
      Then the language server for "java" was sent 2 "workspace/executeCommand" requests

    Scenario: A hosted adapter whose language server is not running is refused by name
      When I start the Launch configuration "orders" from the palette
      Then the editor refuses with "no-language-server"
      And no Debug session exists

    Scenario: A language server that goes away before it answers ends the session
      Given a language server for "java" is ready
      And I open "src/main/java/Orders.java"
      And I start the Launch configuration "orders" from the palette
      When the language server for "java" exits
      Then the editor refuses with "no-language-server"
      And no Debug session exists

    Scenario: A language server that cannot start the adapter is reported
      Given a language server for "java" is ready
      And I open "src/main/java/Orders.java"
      And I start the Launch configuration "orders" from the palette
      When the language server for "java" answers "workspace/executeCommand" with the error "No delegateCommandHandler for vscode.java.startDebugSession"
      Then the editor refuses with "launch-failed"
      And no Debug session exists

    Scenario: A hosted adapter's row is as installed as its language server
      Given the command "jdtls" is on PATH
      When the tools list is shown
      Then the Debug adapter row for "java" is "installed"

  Rule: Launch sessions end with their program, and stop ends everything the adapter opened

    Scenario: A launched program that exits ends the session
      Given a Debug session was started from the Launch configuration "server"
      When the Debug adapter sends the event "terminated"
      Then no Debug session exists

    Scenario: Stopping a launched session terminates the program and its children
      Given a Debug session was started from the Launch configuration "server"
      When I press "Ctrl+F2"
      Then the Debug adapter was sent a "disconnect" request with "terminateDebuggee" true

    Scenario: An adapter that goes away ends the session
      Given a Debug session was started from the Launch configuration "server"
      When the edge reports the Debug adapter is gone
      Then no Debug session exists

  Rule: Child sessions are more threads, never sessions of their own

    Scenario: A child session's threads join the one Debug session
      Given a Debug session was started from the Launch configuration "server"
      When the Debug adapter asks to start a child session whose thread is "worker 1"
      Then the Frames list the thread "worker 1"
      And exactly one Debug session is shown

    Scenario: A child session is never offered as a session to pick
      Given a Debug session was started from the Launch configuration "server"
      When the Debug adapter asks to start a child session whose thread is "worker 1"
      Then no session picker is shown
      And the Transport has one "stop" Chip

    Scenario: The Transport steps the child's thread being inspected
      Given a Debug session was started from the Launch configuration "server"
      And the Debug adapter asks to start a child session whose thread is "worker 1"
      When I press "F8"
      Then the child session was sent a "next" request for its thread
      And the Debug adapter was sent no "next" request

    Scenario: Stopping the session stops its child sessions too
      Given a Debug session was started from the Launch configuration "server"
      And the Debug adapter asks to start a child session whose thread is "worker 1"
      When I press "Ctrl+F2"
      Then every child session was sent a "disconnect" request

  Rule: A session that fails to start says why, and its adapter is installable

    Scenario: A missing adapter is refused by name
      Given the command "codelldb" is not on PATH
      When I start the Launch configuration "server" from the palette
      Then the editor refuses with "no-debug-adapter"
      And no Debug session exists

    Scenario: An adapter that could not be spawned is not a session
      Given I start the Launch configuration "server" from the palette
      When the edge reports the Debug adapter could not be started
      Then the editor refuses with "debug-adapter-failed"
      And no Debug session exists

    Scenario: A launch the adapter rejects is reported
      Given the Debug adapter for "rust" is ready
      And I start the Launch configuration "server" from the palette
      When the Debug adapter answers "launch" with the error "program not found"
      Then the editor refuses with "launch-failed"
      And no Debug session exists

    Scenario: A Launch configuration naming no configured adapter is refused
      Given the project config is:
        """
        [launch.odd]
        adapter = "cobol"
        request = "launch"
        args = {}
        """
      When I start the Launch configuration "odd" from the palette
      Then the editor refuses with "no-debug-adapter"

    Scenario: A missing adapter's row is installable from Tools
      Given the command "codelldb" is not on PATH
      When the tools list is shown
      Then the Debug adapter row for "rust" is "missing"

    Scenario Outline: The JavaScript and TypeScript adapter is offered in Tools to install
      Given the command "js-debug-adapter" is not on PATH
      When the tools list is shown
      Then the Debug adapter row for "<language>" is "available"

      Examples:
        | language   |
        | javascript |
        | typescript |
