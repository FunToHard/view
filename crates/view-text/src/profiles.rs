use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    SingleLineV1,
    PlainMultilineV1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    Mandatory,
    Optional,
    NotApplicable,
}
#[derive(Clone, Copy, Debug)]
pub struct ProfileCase {
    pub id: &'static str,
    pub requirement: Requirement,
    pub native_or_rendered: bool,
}
impl Profile {
    pub fn cases(self) -> Vec<ProfileCase> {
        let mut cases = vec![
            ProfileCase {
                id: "replace-selection",
                requirement: Requirement::Mandatory,
                native_or_rendered: false,
            },
            ProfileCase {
                id: "unicode-delete",
                requirement: Requirement::Mandatory,
                native_or_rendered: false,
            },
            ProfileCase {
                id: "history",
                requirement: Requirement::Mandatory,
                native_or_rendered: false,
            },
            ProfileCase {
                id: "controlled-values",
                requirement: Requirement::Mandatory,
                native_or_rendered: false,
            },
            ProfileCase {
                id: "native-clipboard",
                requirement: Requirement::Mandatory,
                native_or_rendered: true,
            },
            ProfileCase {
                id: "native-ime",
                requirement: Requirement::Mandatory,
                native_or_rendered: true,
            },
            ProfileCase {
                id: "rendered-caret",
                requirement: Requirement::Mandatory,
                native_or_rendered: true,
            },
            ProfileCase {
                id: "native-accessibility",
                requirement: Requirement::Mandatory,
                native_or_rendered: true,
            },
        ];
        cases.push(ProfileCase {
            id: "newline",
            requirement: if self == Self::PlainMultilineV1 {
                Requirement::Mandatory
            } else {
                Requirement::NotApplicable
            },
            native_or_rendered: false,
        });
        cases.push(ProfileCase {
            id: "tab-insertion",
            requirement: if self == Self::PlainMultilineV1 {
                Requirement::Optional
            } else {
                Requirement::NotApplicable
            },
            native_or_rendered: false,
        });
        for id in [
            "navigation-selection",
            "read-only-disabled",
            "clipboard-service",
            "composition",
            "limits-validation",
            "blink-placeholder",
            "word-delete",
        ] {
            cases.push(ProfileCase {
                id,
                requirement: Requirement::Mandatory,
                native_or_rendered: false,
            });
        }
        for id in [
            "bidi-hit-mapping",
            "caret-reveal",
            "lifecycle",
            "command-routing",
        ] {
            cases.push(ProfileCase {
                id,
                requirement: Requirement::Mandatory,
                native_or_rendered: true,
            });
        }
        cases
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaseResult {
    Passed,
    Failed(String),
    Uncovered,
    NotApplicable,
}
#[derive(Clone, Debug)]
pub struct ConformanceReport {
    pub profile: Profile,
    pub adapter: String,
    pub platform: String,
    pub options: EditorConfig,
    pub cases: Vec<(&'static str, CaseResult)>,
}
impl ConformanceReport {
    pub fn certified(&self) -> bool {
        if self
            .cases
            .iter()
            .any(|(_, result)| matches!(result, CaseResult::Failed(_)))
        {
            return false;
        }
        self.profile
            .cases()
            .iter()
            .filter(|c| c.requirement == Requirement::Mandatory)
            .all(|c| {
                self.cases
                    .iter()
                    .any(|(id, result)| *id == c.id && *result == CaseResult::Passed)
            })
    }
}
/// Public adapter for reusable independent edit fixtures; community controls can
/// translate these operations to their public commands without private tree access.
pub trait ProfileAdapter {
    fn configuration(&self) -> EditorConfig;
    fn reset(&mut self, value: &str) -> Result<(), TextError>;
    fn select_range(&mut self, selection: Selection) -> Result<(), TextError>;
    fn insert(&mut self, value: &str) -> Result<(), TextError>;
    fn backspace(&mut self) -> Result<(), TextError>;
    fn undo(&mut self) -> Result<(), TextError>;
    fn value(&self) -> String;
    fn observation(&self) -> EditorSnapshot;
    fn external(&mut self, base: TextRevision, value: &str) -> Result<ValueUpdate, TextError>;
    fn command(&mut self, command: ProfileCommand) -> Result<bool, TextError>;
}
/// Reusable fixtures route these through an adapter's public command surface.
pub enum ProfileCommand {
    Move(Movement, bool),
    Modes { read_only: bool, disabled: bool },
    Paste(String),
    Preedit(String),
    Commit(String),
    Cancel,
    WordDelete,
    Newline,
    Tab,
}
type ProfileCheck = (
    &'static str,
    fn(&mut dyn ProfileAdapter) -> Result<bool, TextError>,
);
pub fn run_profile(
    adapter: &mut impl ProfileAdapter,
    profile: Profile,
    name: &str,
    platform: &str,
) -> ConformanceReport {
    let mut report = ConformanceReport {
        profile,
        adapter: name.into(),
        platform: platform.into(),
        options: adapter.configuration(),
        cases: profile
            .cases()
            .iter()
            .map(|c| {
                (
                    c.id,
                    if c.requirement == Requirement::NotApplicable {
                        CaseResult::NotApplicable
                    } else {
                        CaseResult::Uncovered
                    },
                )
            })
            .collect(),
    };
    let checks: [ProfileCheck; 9] = [
        ("replace-selection", |a| {
            a.reset("abcd")?;
            a.select_range(Selection {
                anchor: TextIndex(3),
                focus: TextIndex(1),
            })?;
            a.insert("X")?;
            Ok(a.value() == "aXd")
        }),
        ("unicode-delete", |a| {
            a.reset("a👩‍💻")?;
            a.select_range(Selection::caret(TextIndex(12)))?;
            a.backspace()?;
            Ok(a.value() == "a")
        }),
        ("history", |a| {
            a.reset("abc")?;
            a.select_range(Selection::caret(TextIndex(3)))?;
            a.insert("Z")?;
            a.undo()?;
            Ok(a.value() == "abc")
        }),
        ("controlled-values", |a| {
            a.reset("abc")?;
            let old = a.observation().text.revision();
            a.select_range(Selection::caret(TextIndex(1)))?;
            a.command(ProfileCommand::Preedit("仮".into()))?;
            let before = a.observation();
            if a.external(old, "abc")? != ValueUpdate::Echo || a.observation() != before {
                return Ok(false);
            }
            a.command(ProfileCommand::Commit("日".into()))?;
            let after = a.observation();
            Ok(a.external(old, "abc") == Err(TextError::StaleRevision) && a.observation() == after)
        }),
        ("navigation-selection", |a| {
            a.reset("abc")?;
            a.command(ProfileCommand::Move(Movement::Forward, false))?;
            a.command(ProfileCommand::Move(Movement::Forward, true))?;
            Ok(a.observation().selection
                == Selection {
                    anchor: TextIndex(1),
                    focus: TextIndex(2),
                })
        }),
        ("read-only-disabled", |a| {
            a.reset("abc")?;
            a.command(ProfileCommand::Modes {
                read_only: true,
                disabled: false,
            })?;
            if a.insert("x") != Err(TextError::ReadOnly) {
                return Ok(false);
            }
            a.select_range(Selection::caret(TextIndex(1)))?;
            a.command(ProfileCommand::Modes {
                read_only: false,
                disabled: true,
            })?;
            Ok(a.insert("x") == Err(TextError::Disabled)
                && a.select_range(Selection::caret(TextIndex(0))) == Err(TextError::Disabled)
                && a.value() == "abc")
        }),
        ("clipboard-service", |a| {
            a.reset("abc")?;
            a.select_range(Selection {
                anchor: TextIndex(1),
                focus: TextIndex(2),
            })?;
            a.command(ProfileCommand::Paste("😀".into()))?;
            Ok(a.value() == "a😀c")
        }),
        ("composition", |a| {
            a.reset("abc")?;
            a.select_range(Selection {
                anchor: TextIndex(1),
                focus: TextIndex(2),
            })?;
            a.command(ProfileCommand::Preedit("日本".into()))?;
            if a.value() != "abc" {
                return Ok(false);
            }
            a.command(ProfileCommand::Cancel)?;
            if a.value() != "abc" || a.observation().composition.is_some() {
                return Ok(false);
            }
            a.command(ProfileCommand::Preedit("日本".into()))?;
            a.command(ProfileCommand::Commit("日".into()))?;
            a.undo()?;
            Ok(a.value() == "abc")
        }),
        ("word-delete", |a| {
            a.reset("one two")?;
            a.command(ProfileCommand::WordDelete)?;
            Ok(a.value() == "two")
        }),
    ];
    for (id, check) in checks {
        let result = match check(adapter) {
            Ok(true) => CaseResult::Passed,
            Ok(false) => CaseResult::Failed("independent expected value differs".into()),
            Err(e) => CaseResult::Failed(e.to_string()),
        };
        if let Some(entry) = report.cases.iter_mut().find(|c| c.0 == id) {
            entry.1 = result;
        }
    }
    if profile == Profile::PlainMultilineV1 {
        for (id, command, expected) in [
            ("newline", ProfileCommand::Newline, "\na"),
            ("tab-insertion", ProfileCommand::Tab, "\ta"),
        ] {
            let result = adapter.reset("a").and_then(|()| adapter.command(command));
            let outcome = match result {
                Ok(true) if adapter.value() == expected => CaseResult::Passed,
                Ok(false) if id == "tab-insertion" => CaseResult::NotApplicable,
                Ok(_) => {
                    CaseResult::Failed("multiline command did not produce expected value".into())
                }
                Err(e) => CaseResult::Failed(e.to_string()),
            };
            if let Some(entry) = report.cases.iter_mut().find(|c| c.0 == id) {
                entry.1 = outcome;
            }
        }
    }
    report
}
impl ProfileAdapter for PlainEditor {
    fn configuration(&self) -> EditorConfig {
        self.config().clone()
    }
    fn reset(&mut self, value: &str) -> Result<(), TextError> {
        *self = Self::new(value, self.config().clone())?;
        Ok(())
    }
    fn select_range(&mut self, selection: Selection) -> Result<(), TextError> {
        EditorSession::select(self, self.snapshot().text.revision(), selection)
    }
    fn insert(&mut self, value: &str) -> Result<(), TextError> {
        PlainEditor::insert(self, value, EditKind::Command, 0)
    }
    fn backspace(&mut self) -> Result<(), TextError> {
        self.delete(true, false, 0)
    }
    fn undo(&mut self) -> Result<(), TextError> {
        PlainEditor::undo(self).map(|_| ())
    }
    fn value(&self) -> String {
        self.snapshot().text.text().into()
    }
    fn observation(&self) -> EditorSnapshot {
        self.snapshot()
    }
    fn external(&mut self, base: TextRevision, value: &str) -> Result<ValueUpdate, TextError> {
        self.external_value(base, value, ExternalPolicy::PreserveSelection)
    }
    fn command(&mut self, command: ProfileCommand) -> Result<bool, TextError> {
        match command {
            ProfileCommand::Move(movement, extend) => self.move_caret(movement, extend, 0)?,
            ProfileCommand::Modes {
                read_only,
                disabled,
            } => self.set_modes(read_only, disabled),
            ProfileCommand::Paste(value) => self.paste(&mut MemoryClipboard(value), 0)?,
            ProfileCommand::Preedit(value) => {
                self.preedit(&value, Selection::caret(TextIndex(value.len())))?
            }
            ProfileCommand::Commit(value) => self.commit_composition(&value, 0)?,
            ProfileCommand::Cancel => self.cancel_composition(),
            ProfileCommand::WordDelete => self.delete(false, true, 0)?,
            ProfileCommand::Newline => return self.newline(0),
            ProfileCommand::Tab => return self.tab(0),
        }
        Ok(true)
    }
}
