//! Reusable specialized preview-v1 adapter. It does not certify native rich-text behavior.
use crate::{Error, MarkdownPreview};
use std::ops::Range;
pub const PROFILE: &str = "view.markdown.block-preview/1";
pub trait SelectablePreview {
    fn plain_text(&self) -> &str;
    fn select(&mut self, range: Range<usize>) -> Result<(), Error>;
    fn copy(&self) -> &str;
}
impl SelectablePreview for MarkdownPreview {
    fn plain_text(&self) -> &str {
        self.plain_text()
    }
    fn select(&mut self, range: Range<usize>) -> Result<(), Error> {
        self.select(range)
    }
    fn copy(&self) -> &str {
        self.copy()
    }
}
/// Checks cross-block copy, rejected out-of-range/interior UTF-8 boundaries and
/// selection survival after rejection. Caller supplies the expected plain text.
pub fn check_selection(preview: &mut impl SelectablePreview, expected: &str) -> Result<(), String> {
    if preview.plain_text() != expected {
        return Err("plain text differs".into());
    }
    preview
        .select(0..expected.len())
        .map_err(|e| format!("select all: {e:?}"))?;
    if preview.copy() != expected {
        return Err("cross-block copy differs".into());
    }
    if preview.select(0..expected.len() + 1).is_ok() {
        return Err("accepted out-of-bounds selection".into());
    }
    for i in 0..expected.len() {
        if !expected.is_char_boundary(i) && preview.select(i..i).is_ok() {
            return Err("accepted interior UTF-8 boundary".into());
        }
    }
    if preview.copy() != expected {
        return Err("rejected selection changed prior selection".into());
    }
    Ok(())
}
