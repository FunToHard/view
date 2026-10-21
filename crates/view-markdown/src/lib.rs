//! Bounded Markdown preview control. See the public control guide for its profile.
#![forbid(unsafe_code)]
pub mod conformance;

use std::ops::Range;
use view::{
    Description, Dirty, Revision, Role, SemanticAction,
    authoring::{Element, IntoElement},
};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub source_bytes: usize,
    pub blocks: usize,
    pub target_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 256 * 1024,
            blocks: 4096,
            target_bytes: 2048,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Limit,
    InvalidSelection,
    Stale,
    InvalidTarget,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Paragraph,
    Heading(u8),
    Code,
    Link(String),
    Image(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub kind: Kind,
    pub text: String,
    pub plain_range: Range<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkAction {
    pub revision: Revision,
    pub target: String,
}
/// Specialized read-only block preview v1, not a rich-text editor or CommonMark parser.
pub struct MarkdownPreview {
    blocks: Vec<Block>,
    plain: String,
    selection: Range<usize>,
    revision: Revision,
    limits: Limits,
}
impl MarkdownPreview {
    pub fn new(source: &str, limits: Limits) -> Result<Self, Error> {
        let (blocks, plain) = parse(source, limits)?;
        Ok(Self {
            blocks,
            plain,
            selection: 0..0,
            revision: Revision::default(),
            limits,
        })
    }
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    pub fn plain_text(&self) -> &str {
        &self.plain
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn selection(&self) -> Range<usize> {
        self.selection.clone()
    }
    /// Selection uses checked UTF-8 scalar boundaries across the plain block text.
    pub fn select(&mut self, range: Range<usize>) -> Result<(), Error> {
        if range.start > range.end
            || range.end > self.plain.len()
            || !self.plain.is_char_boundary(range.start)
            || !self.plain.is_char_boundary(range.end)
        {
            return Err(Error::InvalidSelection);
        }
        self.selection = range;
        Ok(())
    }
    pub fn copy(&self) -> &str {
        &self.plain[self.selection.clone()]
    }
    /// Atomic source replacement resets selection; callers persist document state separately.
    pub fn update(&mut self, source: &str, base: Revision) -> Result<(), Error> {
        if base != self.revision {
            return Err(Error::Stale);
        }
        let (blocks, plain) = parse(source, self.limits)?;
        let revision = self.revision.checked_next().map_err(|_| Error::Limit)?;
        self.blocks = blocks;
        self.plain = plain;
        self.selection = 0..0;
        self.revision = revision;
        Ok(())
    }
    /// Link activation only returns a typed app request. It never opens URLs or programs.
    pub fn activate(&self, block: usize, revision: Revision) -> Result<LinkAction, Error> {
        if revision != self.revision {
            return Err(Error::Stale);
        }
        match self.blocks.get(block).map(|b| &b.kind) {
            Some(Kind::Link(target)) => Ok(LinkAction {
                revision,
                target: target.clone(),
            }),
            _ => Err(Error::InvalidTarget),
        }
    }
    /// Compose through the public runtime contract. Host maps keyboard/pointer activation
    /// to its action type and supplies scrolling/paint using committed bounds and names.
    pub fn element<M, A: 'static>(
        &self,
        key: &str,
        on_link: impl Fn(&mut M, A) -> Dirty + Clone + 'static,
    ) -> Element<M, A> {
        let children = self
            .blocks
            .iter()
            .enumerate()
            .map(|(index, b)| {
                let role = match b.kind {
                    Kind::Heading(_) => Role::Custom("heading".into()),
                    Kind::Link(_) => Role::Custom("link".into()),
                    Kind::Image(_) => Role::Custom("image".into()),
                    _ => Role::StaticText,
                };
                let link = matches!(b.kind, Kind::Link(_));
                let handler = on_link.clone();
                Description::new(
                    format!("{}-{index}", self.revision.get()),
                    (),
                    move |_, m, a| if link { handler(m, a) } else { Dirty::NONE },
                )
                .role(role)
                .name(b.text.clone())
                .focusable(link)
                .semantic_actions(if link {
                    vec![SemanticAction::Focus, SemanticAction::Click]
                } else {
                    vec![]
                })
                .layout(view::LogicalRect::from_xywh(
                    0.,
                    index as f32 * 28.,
                    500.,
                    28.,
                ))
                .into_element()
            })
            .collect::<Vec<_>>();
        Description::new(key, (), |_, _, _| Dirty::NONE)
            .role(Role::Container)
            .name("Markdown preview")
            .into_element()
            .children(children)
    }
}
fn parse(source: &str, limits: Limits) -> Result<(Vec<Block>, String), Error> {
    if source.len() > limits.source_bytes {
        return Err(Error::Limit);
    }
    let mut blocks = vec![];
    let mut plain = String::new();
    let mut code = false;
    for line in source.lines() {
        if line == "```" {
            code = !code;
            continue;
        }
        if line.is_empty() && !code {
            continue;
        }
        if blocks.len() >= limits.blocks {
            return Err(Error::Limit);
        }
        let (kind, text) = if code {
            (Kind::Code, line)
        } else {
            let heading = line.bytes().take_while(|b| *b == b'#').count();
            if (1..=6).contains(&heading) && line.as_bytes().get(heading) == Some(&b' ') {
                (Kind::Heading(heading as u8), &line[heading + 1..])
            } else if let Some((label, target)) =
                target(line.strip_prefix("![").or_else(|| line.strip_prefix('[')))
            {
                if target.len() > limits.target_bytes {
                    return Err(Error::Limit);
                }
                // Schemes are deliberately allowlisted; host still authorizes each navigation.
                if target.starts_with("https://") && !target.chars().any(char::is_control) {
                    (
                        if line.starts_with('!') {
                            Kind::Image(target.into())
                        } else {
                            Kind::Link(target.into())
                        },
                        label,
                    )
                } else {
                    (Kind::Paragraph, line)
                }
            } else {
                (Kind::Paragraph, line)
            }
        };
        if !plain.is_empty() {
            plain.push('\n');
        }
        let start = plain.len();
        plain.push_str(text);
        blocks.push(Block {
            kind,
            text: text.into(),
            plain_range: start..plain.len(),
        });
    }
    Ok((blocks, plain))
}
fn target(line: Option<&str>) -> Option<(&str, &str)> {
    let (label, target) = line?.split_once("](")?;
    Some((label, target.strip_suffix(')')?))
}
