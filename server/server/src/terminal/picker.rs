use tethera_common::protocol::error::WireError;
use tethera_common::protocol::terminal::{Key, Mods, TerminalInput};
use tethera_common::structs::agent::{Agent, ScreenChrome};
use tethera_common::structs::transcript::{Answer, Ask};
use tethera_common::traits::AgentTrait;

/// Answering an agent's question picker, as key presses.
///
/// Every rule here was measured against a live harness rather than read from
/// its help, because none of it is documented and each part of it is the
/// difference between an answer and a keystroke that landed somewhere else:
///
/// - **A choice is made by moving to its row and pressing Enter**, never by
///   pressing the row's number. The harness draws two pickers and only the plain
///   one accepts numbers; the side-by-side layout prints `1. Alpha` and ignores
///   `1` entirely, which is how an answer could be typed into a pane, reported as
///   sent, and never arrive. Arrows and Enter drive both, and the cursor always
///   starts on the first row, so counting from it is not a guess.
/// - A single-select answer **advances to the next question on its own**.
/// - A multi-select toggles with each number and advances with `Right`. It is
///   only ever drawn as the numbered picker, and **cannot meet the picker that
///   ignores numbers**: the side-by-side layout exists to show a preview, and
///   previews are single-select only. So there is no side-by-side checkbox
///   screen to go hunting for.
/// - The free-text row is numbered after the options — two options put "Type
///   something" at 3 — and selecting it opens an editor that takes the text and
///   a `Return`. The side-by-side picker has no such row at all.
/// - A set of more than one answer ends on a review screen, which submits with
///   `1`. A single question submits the moment its row is taken.
pub struct Picker {
    chrome: &'static ScreenChrome,
}

impl Picker {
    /// A driver for a harness somebody has measured.
    ///
    /// `None` for one nobody has. **Every rule above describes one harness**,
    /// and a picker driven on the assumption that a second behaves the same
    /// would press keys at a screen nobody has looked at, take whatever they
    /// selected as the answer, and report that it worked. Refusing leaves a
    /// person answering at the machine, which is where they already were.
    pub fn for_agent(agent: Agent) -> Option<Self> {
        agent.screen_chrome().map(|chrome| Self { chrome })
    }

    /// What the review screen says, and how this server knows one is showing.
    ///
    /// Read rather than predicted. Whether a review appears depends on the shape
    /// of the set, and a rule inferred from today's harness would submit into
    /// whatever is on screen the day that changes.
    pub fn review_marker(&self) -> &'static str {
        self.chrome.review_marker
    }

    /// The key that sends a completed review.
    pub fn submit(&self) -> Key {
        Key::Char(self.chrome.submit)
    }

    /// The highest row a number key can reach.
    ///
    /// A picker is numbered from 1, so beyond the ninth row there is no key to
    /// press. Unreachable with a harness that offers at most four options plus
    /// two rows of its own, and refused rather than mis-pressed if that changes.
    const LAST_NUMBERED_ROW: usize = 9;

    /// The presses that answer a whole set, in order.
    ///
    /// Built before anything is sent, so a set this server cannot express is
    /// refused with the picker untouched. Half-driving it would leave a person's
    /// screen part-answered with no way back.
    /// Takes `&self` so that driving a picker requires having been handed one,
    /// and a harness nobody has measured cannot be handed one at all. The key
    /// sequence below is the measured harness's; a second would make it another
    /// field on `ScreenChrome` rather than a branch here.
    pub fn steps(
        &self,
        asks: &[Ask],
        answers: &[Answer],
        ticks: Option<&[Vec<bool>]>,
    ) -> Result<Vec<TerminalInput>, WireError> {
        if answers.len() != asks.len() {
            return Err(WireError::Backend {
                message: format!(
                    "this question has {} parts and {} answers arrived; the whole set is \
                     answered at once",
                    asks.len(),
                    answers.len()
                ),
            });
        }

        let mut steps = Vec::new();

        for (index, (ask, answer)) in asks.iter().zip(answers).enumerate() {
            let ticked = ticks.and_then(|rows| rows.get(index)).map(Vec::as_slice);

            steps.extend(Self::one(ask, answer, ticked)?);
        }

        Ok(steps)
    }

    fn one(
        ask: &Ask,
        answer: &Answer,
        ticked: Option<&[bool]>,
    ) -> Result<Vec<TerminalInput>, WireError> {
        match answer {
            Answer::Choice(index) => {
                // Moved to rather than numbered. The side-by-side picker prints
                // `1. Alpha` and does nothing whatever when `1` is pressed,
                // which is the whole of why an answer could be typed into a pane,
                // reported as sent, and never arrive.
                Ok(Self::move_to(Self::option_row(ask, *index)?))
            }

            Answer::Multi(indices) if !ask.multi_select => Err(WireError::Backend {
                message: format!(
                    "{:?} was answered with {} choices and takes one",
                    ask.prompt,
                    indices.len()
                ),
            }),

            Answer::Multi(indices) => {
                let mut steps = Vec::new();

                for index in indices {
                    let row = Self::option_row(ask, *index)?;

                    // A number key toggles rather than sets. Pressing a row the
                    // operator had already ticked at the machine would clear the
                    // very row that was chosen, so only a difference is pressed.
                    if ticked.and_then(|rows| rows.get(row - 1)) == Some(&true) {
                        continue;
                    }

                    steps.push(Self::row(row)?);
                }

                // A row left ticked that nobody chose is an answer nobody gave.
                if let Some(rows) = ticked {
                    for (position, _) in rows.iter().enumerate().filter(|(_, on)| **on) {
                        let chosen = indices.iter().any(|index| usize::from(*index) == position);

                        if !chosen {
                            steps.push(Self::row(position + 1)?);
                        }
                    }
                }

                // Toggling leaves the picker on the same question. `Right` is
                // what moves off it, to the next question or to the review.
                steps.push(TerminalInput::Key {
                    key: Key::Right,
                    mods: Mods::NONE,
                });

                Ok(steps)
            }

            Answer::Text(_) if !ask.allows_free_text => Err(WireError::Backend {
                message: format!("{:?} does not take a free-text answer", ask.prompt),
            }),

            Answer::Text(text) if text.trim().is_empty() => Err(WireError::Backend {
                message: format!("{:?} was answered with nothing", ask.prompt),
            }),

            Answer::Text(text) => {
                // Sits after the options, which is where the harness puts it:
                // two options make "Type something" the third row. Reached the
                // same way as any other row, so the picker that ignores numbers
                // is driven correctly too.
                let mut steps = Self::move_to(ask.options.len() + 1);

                steps.push(TerminalInput::Text(text.clone()));
                steps.push(TerminalInput::Key {
                    key: Key::Enter,
                    mods: Mods::NONE,
                });

                Ok(steps)
            }
        }
    }

    /// Walking the cursor from the first row to the nth, then taking it.
    ///
    /// The cursor is on row one when a picker appears, so the number of presses
    /// is known rather than inferred from a position this server cannot see. It
    /// also has no upper bound, unlike a number key, so a picker with more rows
    /// than there are digits stops being unanswerable.
    fn move_to(row: usize) -> Vec<TerminalInput> {
        let mut steps = Vec::new();

        for _ in 1..row {
            steps.push(TerminalInput::Key {
                key: Key::Down,
                mods: Mods::NONE,
            });
        }

        steps.push(TerminalInput::Key {
            key: Key::Enter,
            mods: Mods::NONE,
        });

        steps
    }

    /// The 1-based row an option index sits on.
    fn option_row(ask: &Ask, index: u16) -> Result<usize, WireError> {
        let index = usize::from(index);

        if index >= ask.options.len() {
            return Err(WireError::Backend {
                message: format!(
                    "{:?} has {} options and option {index} was chosen",
                    ask.prompt,
                    ask.options.len()
                ),
            });
        }

        Ok(index + 1)
    }

    fn row(row: usize) -> Result<TerminalInput, WireError> {
        if row > Self::LAST_NUMBERED_ROW {
            return Err(WireError::Backend {
                message: format!(
                    "this machine answers a question by pressing its number, and row {row} \
                     has no number key"
                ),
            });
        }

        Ok(TerminalInput::Key {
            // `row` is at most nine, so this is a digit.
            key: Key::Char(char::from_digit(row as u32, 10).unwrap_or('1')),
            mods: Mods::NONE,
        })
    }
}
