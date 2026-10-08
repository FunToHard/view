use crate::{TextError, TextIndex};
use unicode_segmentation::UnicodeSegmentation;

/// Extended grapheme boundaries, including end-of-text. CRLF is one cluster.
pub fn grapheme_boundaries(text: &str) -> Vec<usize> {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect()
}
pub fn validate_caret(text: &str, index: TextIndex) -> Result<(), TextError> {
    if grapheme_boundaries(text).binary_search(&index.0).is_ok() {
        Ok(())
    } else {
        Err(TextError::InvalidIndex)
    }
}
pub fn previous_grapheme(text: &str, index: usize) -> usize {
    grapheme_boundaries(text)
        .into_iter()
        .rev()
        .find(|&i| i < index)
        .unwrap_or(0)
}
pub fn next_grapheme(text: &str, index: usize) -> usize {
    grapheme_boundaries(text)
        .into_iter()
        .find(|&i| i > index)
        .unwrap_or(text.len())
}
/// Word movement uses Unicode word starts: punctuation/whitespace are skipped.
pub fn previous_word(text: &str, index: usize) -> usize {
    text.unicode_word_indices()
        .map(|(i, _)| i)
        .rfind(|&i| i < index)
        .unwrap_or(0)
}
pub fn next_word(text: &str, index: usize) -> usize {
    text.unicode_word_indices()
        .map(|(i, _)| i)
        .find(|&i| i > index)
        .unwrap_or(text.len())
}
/// Select the Unicode word under an index, otherwise the grapheme under it.
pub fn word_range(text: &str, index: TextIndex) -> Result<std::ops::Range<usize>, TextError> {
    validate_caret(text, index)?;
    for (start, word) in text.unicode_word_indices() {
        if start <= index.0 && index.0 < start + word.len() {
            return Ok(start..start + word.len());
        }
    }
    Ok(index.0..next_grapheme(text, index.0))
}
