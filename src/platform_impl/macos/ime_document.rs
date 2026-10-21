//! The text that `WinitView` shows input methods through `NSTextInputClient`.
//!
//! winit doesn't see the app's text, so the view reports the text inserted since the caret last
//! moved for another reason (a command key, a click, a shortcut, focus), with the caret at its
//! end and the marked text after it. That is enough for the replacements input methods make next
//! to the caret: press-and-hold replaces the letter just typed with an accented one, and dictation
//! replaces the words it just inserted. Dictation also needs a caret to insert at at all.
//!
//! Offsets are UTF-16 code units, as in `NSRange`. They keep growing across resets, so a range an
//! input method computed before a reset never matches text inserted after it.

/// Text kept for replacements, in characters.
const KEPT_CHARS: usize = 256;

#[derive(Debug, Default)]
pub(super) struct ImeDocument {
    /// The text inserted since the last reset; the caret is at its end.
    typed: String,
    /// The offset of `typed` in the document.
    start: usize,
}

impl ImeDocument {
    /// The caret's offset, which is also where marked text starts.
    pub fn caret(&self) -> usize {
        self.start + utf16_len(&self.typed)
    }

    /// Forget the text: the caret may have moved.
    pub fn reset(&mut self) {
        self.start = self.caret();
        self.typed.clear();
    }

    /// Text reached the app at the caret.
    pub fn insert(&mut self, text: &str) {
        self.typed.push_str(text);
        let excess = self.typed.chars().count().saturating_sub(KEPT_CHARS);
        if excess > 0 {
            let cut = self.typed.char_indices().nth(excess).map_or(self.typed.len(), |(i, _)| i);
            self.start += utf16_len(&self.typed[..cut]);
            self.typed.drain(..cut);
        }
    }

    /// The selection to report: the caret, or the selection within the marked text.
    pub fn selected_range(&self, marked_selection: Option<(usize, usize)>) -> (usize, usize) {
        match marked_selection {
            Some((location, length)) => (self.caret() + location, length),
            None => (self.caret(), 0),
        }
    }

    /// The known text in `range`, with the range it actually covers.
    pub fn substring(
        &self,
        (location, length): (usize, usize),
    ) -> Option<(String, (usize, usize))> {
        let from = location.max(self.start);
        let to = location.saturating_add(length).min(self.caret());
        if from >= to {
            return None;
        }
        let from_byte = self.byte_offset(from);
        let to_byte = self.byte_offset(to);
        let text = &self.typed[from_byte..to_byte];
        let from = self.start + utf16_len(&self.typed[..from_byte]);
        Some((text.to_owned(), (from, utf16_len(text))))
    }

    /// Remove the text an input method replaces, and return it: the app has to delete it.
    ///
    /// Only a range that reaches the caret is honoured. The part of it after the caret is the
    /// marked text, which the new text replaces anyway. A range that ends before the caret, or
    /// starts before the known text, would need the app's own text and is ignored.
    pub fn replace(&mut self, range: Option<(usize, usize)>) -> String {
        let Some((location, length)) = range else {
            return String::new();
        };
        let caret = self.caret();
        if location >= caret || location < self.start || location.saturating_add(length) < caret {
            return String::new();
        }
        let from = self.byte_offset(location);
        self.typed.split_off(from)
    }

    /// The byte offset in `typed` of a document offset, moved back to a character boundary.
    fn byte_offset(&self, offset: usize) -> usize {
        let mut units = self.start;
        for (index, c) in self.typed.char_indices() {
            units += c.len_utf16();
            if units > offset {
                return index;
            }
        }
        self.typed.len()
    }
}

fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// What to do with text an input method inserts while there's no composition to commit.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PlainInsert {
    /// The key press being handled reports the text.
    KeyText,
    /// Commit the text: no `KeyboardInput` event carries it.
    Commit,
    /// Drop it.
    Ignore,
}

/// `key_characters` is `Some` while `keyDown:` handles a key, with the characters it types.
pub(super) fn plain_insert(text: &str, key_characters: Option<&str>) -> PlainInsert {
    if text.is_empty() || text.chars().next().is_some_and(char::is_control) {
        return PlainInsert::Ignore;
    }
    match key_characters {
        // An input method that types other text than the key, such as Chinese punctuation (`，`
        // for `,`) or an ideographic space for Space, without a composition.
        Some(characters) if characters != text => PlainInsert::Commit,
        Some(_) => PlainInsert::KeyText,
        // The emoji picker or dictation, outside any key press.
        None => PlainInsert::Commit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> ImeDocument {
        let mut document = ImeDocument::default();
        document.insert(text);
        document
    }

    #[test]
    fn press_and_hold_replaces_the_letter_before_the_caret() {
        let mut document = typed("café");
        assert_eq!(document.selected_range(None), (4, 0));
        assert_eq!(document.substring((3, 1)), Some(("é".to_owned(), (3, 1))));
        assert_eq!(document.replace(Some((3, 1))), "é");
        document.insert("è");
        assert_eq!(document.substring((0, 10)), Some(("cafè".to_owned(), (0, 4))));
    }

    #[test]
    fn a_range_into_the_marked_text_replaces_up_to_the_caret() {
        let mut document = typed("e");
        assert_eq!(document.selected_range(Some((1, 0))), (2, 0));
        assert_eq!(document.replace(Some((0, 2))), "e");
        assert_eq!(document.caret(), 0);
    }

    #[test]
    fn ranges_the_document_cannot_answer_are_ignored() {
        let mut document = typed("abc");
        for range in [None, Some((3, 0)), Some((3, 2)), Some((0, 1)), Some((1, 1))] {
            assert_eq!(document.replace(range), "", "{range:?}");
        }
        document.reset();
        document.insert("x");
        // A range from before the reset.
        assert_eq!(document.replace(Some((2, 1))), "");
        assert_eq!(document.substring((0, 3)), None);
        assert_eq!(document.replace(Some((3, 1))), "x");
    }

    #[test]
    fn offsets_count_utf16_units() {
        let mut document = typed("a😀b");
        assert_eq!(document.caret(), 4);
        // A range starting inside the surrogate pair takes the whole character.
        assert_eq!(document.replace(Some((2, 2))), "😀b");
        assert_eq!(document.caret(), 1);
    }

    #[test]
    fn old_text_is_dropped_but_offsets_stay() {
        let mut document = typed(&"a".repeat(KEPT_CHARS));
        document.insert("bc");
        assert_eq!(document.caret(), KEPT_CHARS + 2);
        assert_eq!(document.substring((0, 2)), None);
        assert_eq!(document.replace(Some((KEPT_CHARS, 2))), "bc");
    }

    #[test]
    fn text_that_differs_from_the_key_is_committed() {
        assert_eq!(plain_insert("，", Some(",")), PlainInsert::Commit);
        assert_eq!(plain_insert("\u{3000}", Some(" ")), PlainInsert::Commit);
        assert_eq!(plain_insert("a", Some("a")), PlainInsert::KeyText);
        assert_eq!(plain_insert("😀", None), PlainInsert::Commit);
        assert_eq!(plain_insert("\r", Some("\r")), PlainInsert::Ignore);
        assert_eq!(plain_insert("", None), PlainInsert::Ignore);
    }
}
