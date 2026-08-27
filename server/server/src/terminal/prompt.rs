use tethera_common::structs::agent::{Agent, ScreenChrome};
use tethera_common::structs::transcript::{Ask, Question, QuestionOption};
use tethera_common::structs::ids::QuestionId;
use tethera_common::traits::AgentTrait;

/// A question an agent has on screen and nowhere else.
///
/// A permission prompt is never written to the records — the tool call is, but
/// the request to allow it is drawn, answered and gone. So the only way to know
/// a person is being asked is to read what the agent is showing them.
///
/// Both shapes an agent draws are the same shape underneath: a line asking
/// something, then rows numbered from one. That is why one detector covers a
/// permission prompt and an `AskUserQuestion` picker, and why one answer path
/// drives both — measured, on a live harness, against the two screens committed
/// under `tests/fixtures/screens/`.
///
/// **Conservative by construction.** Every rule below is a reason to return
/// nothing. A false negative leaves a person answering at the machine, which is
/// where they were already; a false positive puts a question on somebody's phone
/// that their keystrokes cannot answer, and an `Unblocked` that never comes.
pub struct PromptDetector {
    chrome: &'static ScreenChrome,
}

impl PromptDetector {
    /// A detector for a harness somebody has measured.
    ///
    /// `None` for one nobody has, and that is the whole answer rather than a
    /// gap to fill in later: every value below describes what one harness draws,
    /// and guessing that a second draws the same would put a question on
    /// somebody's phone that their keystrokes cannot answer.
    pub fn for_agent(agent: Agent) -> Option<Self> {
        agent.screen_chrome().map(|chrome| Self { chrome })
    }

    /// Rows this detector will consider. A picker deeper than this is one it
    /// does not recognise, and guessing at it would answer the wrong row.
    const MAX_ROWS: usize = 9;

    /// How far above the first row to look for what is being asked.
    ///
    /// The prompt sits directly above the options, separated by at most a blank
    /// line. Searching further up reaches the conversation, and a detector that
    /// took a sentence from the transcript as the question would ask a person
    /// something the agent never asked.
    const PROMPT_WITHIN: usize = 3;

    /// What the agent is asking, if it is asking anything.
    pub fn detect(&self, screen: &str) -> Option<Question> {
        let lines: Vec<&str> = screen.lines().collect();
        let (first_row, mut options, closed_by_rule) = self.options(&lines)?;

        // The side-by-side picker has no free-text row to lift, so lifting one
        // there removes a real option and offers a field the screen does not
        // have. Measured against both layouts on a live agent.
        let allows_free_text = !self.is_side_by_side(screen)
            && Self::lift_free_text(&mut options, closed_by_rule);

        // A row closed by a blank line carries an empty description as a marker.
        // It is bookkeeping, not something a person should be shown.
        for option in &mut options {
            if option.description.as_deref() == Some("") {
                option.description = None;
            }
        }

        if options.len() < 2 {
            return None;
        }

        let prompt = self.prompt_above(&lines, first_row)?;
        let asks = vec![Ask {
            header: None,
            prompt,
            options,
            // A screen-drawn prompt is one choice. A multi-select picker draws
            // checkboxes, and this detector does not read them - so it declines
            // to claim a shape it cannot answer.
            multi_select: false,
            // Not a row. A person types into a field the client draws, and the
            // answer comes back as `Answer::Text` — which the picker already
            // knows how to deliver.
            allows_free_text,
        }];

        let fingerprint = Question::fingerprint_of(&asks);

        Some(Question {
            // Derived from the fingerprint, so the same prompt on screen is the
            // same id for as long as it is up, and a different prompt is a
            // different question. Nothing else about a screen is stable enough
            // to be an identity.
            id: QuestionId::mint(fingerprint.0.as_str()),
            fingerprint,
            asks,
        })
    }

    /// The numbered rows, and the line the first of them is on.
    ///
    /// Numbered from one and consecutive. A list that starts at two, or skips a
    /// number, is not a picker this can drive: the number pressed is the row
    /// selected, so a gap would select the wrong one.
    fn options(&self, lines: &[&str]) -> Option<(usize, Vec<QuestionOption>, bool)> {
        let mut first_row = None;
        let mut last_row = 0;
        let mut options: Vec<QuestionOption> = Vec::new();
        let mut closed_by_rule = false;

        for (index, line) in lines.iter().enumerate() {
            // A rule closes the question's own list. What the harness draws
            // below it is its own chrome — the row a person tapped and got
            // "what would you like to clarify?" was one of those, offered to
            // them as if it were an answer.
            if first_row.is_some() && self.is_furniture(line) {
                closed_by_rule = true;

                break;
            }

            let Some((number, label)) = self.numbered(line) else {
                // An indented line directly under a row is that row's
                // description, wrapped. A line after a blank one is not: the
                // blank ended the row, and what follows is the footer.
                if first_row.is_some() && !line.trim().is_empty() {
                    Self::absorb_continuation(&mut options, line);
                } else if first_row.is_some() {
                    Self::close_row(&mut options);
                }

                continue;
            };

            if number != options.len() + 1 {
                // A second list further down the screen. The first one is the
                // one being asked.
                if first_row.is_some() {
                    break;
                }

                continue;
            }

            if options.len() >= Self::MAX_ROWS {
                return None;
            }

            first_row.get_or_insert(index);
            last_row = index;
            options.push(QuestionOption {
                label,
                description: None,
            });
        }

        let at = first_row?;

        if self.composer_below(lines, last_row) {
            return None;
        }

        Some((at, options, closed_by_rule))
    }

    /// Whether this is the picker that draws a preview beside its options.
    ///
    /// **The two layouts do not take the same keys.** The side-by-side one
    /// prints its rows numbered and then ignores the numbers entirely, which is
    /// why an answer could be typed into a pane, reported as sent, and never
    /// arrive. It also carries no free-text row, so the last row inside its list
    /// is a real choice rather than an affordance.
    ///
    /// Recognised by the notes affordance, which belongs to the preview pane and
    /// appears nowhere else. The numbering, the rule and the row shapes are all
    /// identical between the two layouts, so none of them can tell them apart.
    fn is_side_by_side(&self, screen: &str) -> bool {
        screen.contains(self.chrome.preview_hint)
    }

    /// Takes the free-text row out of the options, saying so instead.
    ///
    /// **The row is not a choice and must never be offered as one.** Reported as
    /// an option it is a control a tap cannot fulfil: tapping it selects the
    /// row, the harness opens its own text field, and every later tap types that
    /// row's *number* into the field. An operator hit exactly that and could not
    /// get out of it — "I press 4 or 3 and it just fills in with numbers".
    ///
    /// Identified by shape rather than by its words, because a list of phrases
    /// would rot and would be wrong in another language: it is the last row
    /// inside a list that a rule closes. A permission prompt draws no rule under
    /// its rows and has no free-text row, so its last option — the refusal — is
    /// never touched. That distinction is load-bearing: a machine that could only
    /// ever say yes would be worse than one that could not answer at all.
    fn lift_free_text(options: &mut Vec<QuestionOption>, closed_by_rule: bool) -> bool {
        // Below three, removing one leaves a list that is not a choice. The
        // detector refuses such a screen anyway, and refusing is safer than
        // guessing which of two rows was the affordance.
        if !closed_by_rule || options.len() < 3 {
            return false;
        }

        options.pop();

        true
    }

    /// Stops the newest row taking any more description.
    ///
    /// A blank line ends a row. Without this the footer under a picker becomes
    /// the last option's description, and a person is shown "Enter to select ·
    /// up/down to navigate · Esc to cancel" as though it described a choice.
    fn close_row(options: &mut Vec<QuestionOption>) {
        if let Some(last) = options.last_mut() {
            last.description.get_or_insert_with(String::new);
        }
    }

    /// An indented line under a row is that row's description, wrapped.
    fn absorb_continuation(options: &mut [QuestionOption], line: &str) {
        let text = line.trim();

        if text.is_empty() {
            return;
        }

        let Some(last) = options.last_mut() else {
            return;
        };

        // A row whose description was already closed by a blank line takes no
        // more: what follows the blank belongs to the harness, not to the row.
        if last.description.as_deref() == Some("") {
            return;
        }

        match &mut last.description {
            Some(existing) => {
                existing.push(' ');
                existing.push_str(text);
            }
            None => last.description = Some(text.to_string()),
        }
    }

    /// The question, taken from just above the rows.
    fn prompt_above(&self, lines: &[&str], first_row: usize) -> Option<String> {
        lines[..first_row]
            .iter()
            .rev()
            .take(Self::PROMPT_WITHIN)
            .map(|line| line.trim())
            .find(|line| !line.is_empty() && !self.is_furniture(line))
            .map(str::to_string)
    }

    /// `❯ 1. Yes` and `  2. No` both, with the number and the label.
    fn numbered(&self, line: &str) -> Option<(usize, String)> {
        let text = line.trim_start().trim_start_matches(self.chrome.cursor).trim_start();
        let (digits, rest) = text.split_at(text.find('.')?);
        let number: usize = digits.parse().ok()?;

        let label = Self::before_the_preview(rest.strip_prefix('.')?).trim();

        if label.is_empty() {
            return None;
        }

        Some((number, label.to_string()))
    }

    /// The part of a row that is the option, not the pane drawn beside it.
    ///
    /// The side-by-side picker puts a preview on the **same screen line** as the
    /// option it belongs to:
    ///
    /// ```text
    /// 1. Refuse and say so            ┌──────────────────────────────┐
    /// ```
    ///
    /// Taking the whole line gives a person a label with box art trailing off
    /// the end of it, and makes two options that differ only in their preview
    /// look like different questions - which moves the fingerprint, and a moved
    /// fingerprint refuses the answer sent against it.
    ///
    /// Cut at the box drawing rather than at a column, because where the pane
    /// starts depends on the width of the terminal it is drawn in.
    fn before_the_preview(row: &str) -> &str {
        match row.find(Self::is_box_drawing) {
            Some(at) => &row[..at],
            None => row,
        }
    }

    /// The Unicode block the harness draws its frames from. Nothing in an
    /// option's own text comes from it.
    fn is_box_drawing(character: char) -> bool {
        matches!(character, '\u{2500}'..='\u{257f}')
    }

    /// A rule, or a line that is only the harness's own box drawing.
    fn is_furniture(&self, line: &str) -> bool {
        let text = line.trim();

        !text.is_empty() && text.chars().all(|c| c == self.chrome.rule)
    }

    /// Whether the harness is drawing its input line *below* these rows.
    ///
    /// **While an agent is asking, its composer is not on the screen** — the
    /// picker takes the bottom of the screen and gives it back only once the
    /// question is answered. Measured on a live pane: the same agent idle draws
    /// a rule, an input line and a status bar; holding a picker draws neither
    /// rule nor input line, ending instead on the rows and their hint.
    ///
    /// So a cursor below the last row means the rows are scrollback and
    /// something else owns the bottom. An operator sent a numbered list as an
    /// ordinary message, the harness echoed it behind the same cursor glyph a
    /// picker marks its current row with, and it was published to their phone
    /// as a question — with the line above it as the prompt, and no keystroke
    /// that could answer it. It also never cleared, because scrollback does not
    /// move.
    ///
    /// Scanned from below the *last* row rather than the first, because the
    /// cursor sits on whichever row the person has navigated to, and that row
    /// is one of these.
    fn composer_below(&self, lines: &[&str], last_row: usize) -> bool {
        lines
            .iter()
            .skip(last_row + 1)
            .any(|line| line.trim_start().starts_with(self.chrome.cursor))
    }
}
