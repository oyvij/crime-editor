Feature: Diagnostics under the code they name

  The gutter bar says a line has something wrong with it and the footer says what is wrong with the
  line the cursor is on. Neither says *which characters* the server pointed at, so a reader looking
  at a line with three calls on it still has to guess which one the server meant. The underline is
  the server's own range drawn where it belongs, and the box beside the line is that range's message
  read without moving the cursor onto it.

  The span is clamped to the characters the line actually holds. A server's range outruns the buffer
  routinely — it names a document version the reader has typed past, or it ends on a later line
  altogether — and columns drawn past the end of a line are an underline under nothing.

  Where the pointer rests is a fact only the edge can observe, so it arrives as an event like any
  other and the box is placed by the core: the same `Placement` the hover box and the candidate list
  are placed by, so a box beside a line is never a rectangle the renderer worked out for itself.

  Background:
    Given the workspace root is "/home/me/projects/crime"
    And the project is a git repository
    And a language server for "rust" is ready
    And "src/lib.rs" is open in the editor holding:
      """
      fn main() {
          let x = nope();
      }
      """

  Rule: A diagnostic underlines the characters it names

    Scenario: The span the server pointed at is underlined
      When the language server for "rust" publishes diagnostics for "src/lib.rs" with spans:
        | line | from | to | severity | message           |
        | 2    | 13   | 16 | error    | cannot find value |
      Then the underline on line 2 of "src/lib.rs" covers columns 13 through 16
      And line 1 of "src/lib.rs" has no underline

    Scenario: A span running past the end of the line stops at the last character
      When the language server for "rust" publishes diagnostics for "src/lib.rs" with spans:
        | line | from | to  | severity | message           |
        | 2    | 13   | 400 | error    | cannot find value |
      Then the underline on line 2 of "src/lib.rs" covers columns 13 through 19

  Rule: Resting the pointer on an underline says what is wrong there

    Scenario: The message appears beside the line the underline is on
      Given the language server for "rust" publishes diagnostics for "src/lib.rs" with spans:
        | line | from | to | severity | message           |
        | 2    | 13   | 16 | error    | cannot find value |
      When the pointer rests on line 2 column 14 of the editor
      Then the diagnostic box says "cannot find value"
      And the diagnostic box sits beside line 2

    Scenario: Resting on the same line but off the span says nothing
      Given the language server for "rust" publishes diagnostics for "src/lib.rs" with spans:
        | line | from | to | severity | message           |
        | 2    | 13   | 16 | error    | cannot find value |
      When the pointer rests on line 2 column 6 of the editor
      Then no diagnostic box is shown

    Scenario: Moving the pointer out of the editor takes the box down
      Given the language server for "rust" publishes diagnostics for "src/lib.rs" with spans:
        | line | from | to | severity | message           |
        | 2    | 13   | 16 | error    | cannot find value |
      And the pointer rests on line 2 column 14 of the editor
      When the pointer rests on row 2 column 2 of the file tree
      Then no diagnostic box is shown

    Scenario: The worst of two overlapping diagnostics is the one read out
      Given the language server for "rust" publishes diagnostics for "src/lib.rs" with spans:
        | line | from | to | severity | message           |
        | 2    | 13   | 16 | warning  | unused variable   |
        | 2    | 13   | 16 | error    | cannot find value |
      When the pointer rests on line 2 column 14 of the editor
      Then the diagnostic box says "cannot find value"
