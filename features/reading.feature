Feature: Reading aloud

  A passage is selected and Varde reads it. The point is not to replace reading but
  to sit alongside it: a long document is easy to zone out of, and a voice sets a
  pace the eye can follow.

  A Reading covers the Selection and nothing else. It is spoken as Utterances, one
  per sentence, so that skipping moves by sentence and the sentence being spoken can
  be marked — and it is built as one continuous stream with real silence between
  them, because a player per sentence leaves a gap the machine chose rather than one
  the listener needs.

  What is asserted here is which text reaches the voice, which Utterance is current,
  and what Varde refuses. Never the audio, never the glyphs on the Transport, and
  never a real synthesizer: the voice is an installed binary and the effect that runs
  it is a value. The arguments are in
  `docs/adr/0013-a-voice-is-an-installed-binary.md` and
  `docs/adr/0014-scratch-audio-lives-outside-the-workspace.md`.

  Background:
    Given the workspace root is "/home/me/projects/varde"
    And a voice is configured

  Scenario: A reading covers the selection
    Given "guide.md" is open in the editor holding:
      """
      # Setup

      Install it. Then run it.
      """
    And the selection covers "Install it. Then run it."
    When a reading is started
    Then the reading is in flight
    And the spoken text is "Install it. Then run it."

  Scenario: A passage picked in a preview is read as it is drawn
    Given "guide.md" is open in the editor holding:
      """
      ## Setup

      Install it. Then run it.
      """
    And I drag across the row holding "Install it. Then run it."
    When a reading is started
    Then the reading is in flight
    And the spoken text is "Install it. Then run it."

  Scenario: A passage extended with the keyboard in a preview is read too
    Given "guide.md" is open in the editor holding:
      """
      Install it.
      """
    And I hold shift and alt and press the Right arrow in the editor
    When a reading is started
    Then the reading is in flight
    And the spoken text is "Install"

  Scenario: A reading with nothing selected refuses
    Given "guide.md" is open in the editor holding:
      """
      Install it.
      """
    And there is no selection
    When a reading is started
    Then the reading refuses with "nothing-selected"
    And no reading is in flight

  Scenario Outline: Markdown syntax is never spoken
    Given "guide.md" is open in the editor holding "<source>"
    And the selection covers the whole buffer
    When a reading is started
    Then the spoken text is "<spoken>"

    Examples:
      | source                          | spoken                  |
      | # Setup                         | Setup                   |
      | Install **now**.                | Install now.            |
      | See [the guide](http://x.test). | See the guide.          |
      | - one                           | one                     |
      | > Careful.                      | Careful.                |

  Scenario: Code is read like any other block
    Given "guide.md" is open in the editor holding:
      """
      Run this:

      ```sh
      cargo test
      ```
      """
    And the selection covers the whole buffer
    When a reading is started
    Then the spoken text is "Run this: cargo test"

  Scenario Outline: Only a markdown buffer can be read
    Given "<file>" is open in the editor holding "Install it."
    And the selection covers the whole buffer
    When a reading is started
    Then the reading is <outcome>

    Examples:
      | file        | outcome                  |
      | guide.md    | in flight                |
      | src/main.rs | refused as "not-markdown" |

  Scenario: A reading is one utterance per sentence
    Given "guide.md" is open in the editor holding:
      """
      Install it. Then run it. Read the output.
      """
    And the selection covers the whole buffer
    When a reading is started
    Then the reading holds 3 utterances
    And utterance 1 is "Install it."
    And utterance 3 is "Read the output."

  Scenario: The utterance being spoken is the current one
    Given a reading of "Install it. Then run it. Read the output." is in flight
    When the reading has been speaking for the length of 2 utterances
    Then the current utterance is 3

  Scenario: Next moves one utterance
    Given a reading of "Install it. Then run it. Read the output." is in flight
    And the current utterance is 1
    When the next utterance is asked for
    Then the current utterance is 2
    And the reading is in flight

  Scenario: Previous at the first utterance stays there
    Given a reading of "Install it. Then run it." is in flight
    And the current utterance is 1
    When the previous utterance is asked for
    Then the current utterance is 1

  Scenario: Starting a reading replaces the one in flight
    Given a reading of "Install it." is in flight
    And the selection covers "Then run it."
    When a reading is started
    Then the reading is in flight
    And the spoken text is "Then run it."
    And exactly one reading is in flight

  Scenario: Play starts a reading of the selection when none is in flight
    Given "guide.md" is open in the editor holding "Install it."
    And the selection covers the whole buffer
    When the play control is pressed
    Then the reading is in flight
    And the spoken text is "Install it."

  Scenario: Play with nothing selected refuses out loud rather than doing nothing
    Given "guide.md" is open in the editor holding "Install it."
    And there is no selection
    When the play control is pressed
    Then the reading refuses with "nothing-selected"
    And no reading is in flight

  Scenario: Pausing and resuming continues where it stopped
    Given a reading of "Install it. Then run it." is in flight
    When the reading is paused
    Then the reading is paused
    And no sound is being played
    When the reading is resumed
    Then the reading is in flight
    And the reading resumes from where it was paused

  Scenario: Stopping a reading ends it
    Given a reading of "Install it." is in flight
    When the reading is stopped
    Then no reading is in flight
    And no sound is being played

  Scenario Outline: Speed is a multiplier and higher is faster
    Given the reading speed is <speed>
    When a reading of "Install it." is started
    Then the voice is asked for a duration scale of <scale>

    Examples:
      | speed | scale |
      | 0.90  | 1.11  |
      | 1.00  | 1.00  |
      | 1.25  | 0.80  |

  Scenario: A speed change applies to the next reading
    Given a reading of "Install it. Then run it." is in flight at speed 1.00
    When the reading speed is changed to 1.25
    Then the reading in flight is still at speed 1.00
    When a reading is started
    Then the reading is at speed 1.25

  Scenario Outline: A missing piece refuses out loud and offers its speech row
    Given <missing>
    And "guide.md" is open in the editor holding "Install it."
    And the selection covers the whole buffer
    When a reading is started
    Then the reading refuses with "<slug>"
    And the speech row for "<row>" is offered
    And nothing is waiting on the terminal's input line
    And no command has been executed

    Examples:
      | missing                            | slug              | row         |
      | no synthesizer is on the PATH      | no-synthesizer    | synthesizer |
      | no voice is configured             | no-voice          | synthesizer |
      | the voice file is not on disk      | no-voice          | synthesizer |
      | no audio player is configured      | no-player         | player      |

  Scenario: One key takes the offered speech row, the same act as in Tools
    Given no voice is configured
    And the command "fetch-a-voice" is on PATH
    And "guide.md" is open in the editor holding "Install it."
    And the selection covers the whole buffer
    And a reading was refused
    When I press "i" in the palette
    Then the shell pane runs "fetch-a-voice" reporting its exit status

  Scenario: Reading writes nothing into the workspace
    Given "guide.md" is open in the editor holding "Install it."
    And the selection covers the whole buffer
    When a reading is started
    Then no file was written inside the workspace root
    And the file tree is unchanged

  Scenario: The stream is gone once the reading ends
    Given a reading of "Install it." is in flight
    When the reading is stopped
    Then no stream file remains

  Scenario: The utterance being spoken is marked
    Given "guide.md" is open in the editor holding:
      """
      Install it.

      Then run it.
      """
    And the selection covers the whole buffer
    When a reading is started
    Then the marked lines are 1 to 1
    When the reading has been speaking for the length of 1 utterances
    Then the marked lines are 3 to 3

  Scenario: The mark follows a skip
    Given "guide.md" is open in the editor holding:
      """
      Install it.

      Then run it.
      """
    And a reading of the whole buffer is in flight
    And the current utterance is 1
    When the next utterance is asked for
    Then the marked lines are 3 to 3
    When the previous utterance is asked for
    Then the marked lines are 1 to 1

  Scenario: Nothing is marked once the reading ends
    Given a reading of "Install it." is in flight
    Then the marked lines are 1 to 1
    When the reading is stopped
    Then no lines are marked

  Scenario Outline: The transport belongs to markdown
    Given "<file>" is open in the editor
    Then the transport is <shown>

    Examples:
      | file        | shown       |
      | guide.md    | on screen   |
      | src/main.rs | not on screen |

  Scenario: Every transport action has a key binding
    Then every transport action is reachable from the keyboard
