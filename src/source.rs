use {
    crate::{fallback::Span, LineColumn},
    alloc::vec,
    core::ops::Range,
    std::{
        collections::BTreeMap,
        hash::Hash,
        sync::{Arc, LazyLock, Mutex},
        vec::Vec,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct SourceInner {
    #[cfg_attr(feature = "serde", serde(with = "serde_rc"))]
    name: Arc<str>,
    #[cfg_attr(feature = "serde", serde(with = "serde_rc"))]
    source_text: Arc<str>,
    lines: Vec<usize>,
    chars: usize,
    char_index_to_byte_offset: Mutex<BTreeMap<usize, usize>>,
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Source(#[cfg_attr(feature = "serde", serde(with = "serde_rc"))] Arc<SourceInner>);

pub(crate) static EMPTY_SOURCE: LazyLock<Source> = LazyLock::new(|| Source::new("", ""));

impl Hash for Source {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for Source {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Source {}

impl Source {
    pub fn new(name: &str, source_text: &str) -> Self {
        let (chars, lines) = lines_offsets(source_text);
        Self(Arc::new(SourceInner {
            name: name.into(),
            source_text: source_text.into(),
            lines,
            chars,
            char_index_to_byte_offset: Mutex::default(),
        }))
    }

    pub fn name(&self) -> &Arc<str> {
        &self.0.name
    }

    pub fn source_text(&self) -> &Arc<str> {
        &self.0.source_text
    }

    pub(crate) fn offset_line_column(&self, offset: usize) -> LineColumn {
        assert!(offset <= self.0.chars);
        match self.0.lines.binary_search(&offset) {
            Ok(found) => LineColumn {
                line: found + 1,
                column: 0,
            },
            Err(idx) => LineColumn {
                line: idx,
                column: offset - self.0.lines[idx - 1],
            },
        }
    }

    pub(crate) fn byte_range(&self, span: &Span) -> Range<usize> {
        self.byte(span.lo)..self.byte(span.hi)
    }

    fn byte(&self, ch: u32) -> usize {
        let char_index = ch as usize;

        let mut char_index_to_byte_offset = self.0.char_index_to_byte_offset.lock().unwrap();

        // Look up offset of the largest already-computed char index that is
        // less than or equal to the current requested one.
        let (&previous_char_index, &previous_byte_offset) = char_index_to_byte_offset
            .range(..=char_index)
            .next_back()
            .unwrap_or((&0, &0));

        if previous_char_index == char_index {
            return previous_byte_offset;
        }

        // Look up next char index that is greater than the requested one. We
        // resume counting chars from whichever point is closer.
        let byte_offset = match char_index_to_byte_offset.range(char_index..).next() {
            Some((&next_char_index, &next_byte_offset))
                if next_char_index - char_index < char_index - previous_char_index =>
            {
                self.0.source_text[..next_byte_offset]
                    .char_indices()
                    .nth_back(next_char_index - char_index - 1)
                    .unwrap()
                    .0
            }
            _ => {
                match self.0.source_text[previous_byte_offset..]
                    .char_indices()
                    .nth(char_index - previous_char_index)
                {
                    Some((byte_offset_from_previous, _ch)) => {
                        previous_byte_offset + byte_offset_from_previous
                    }
                    None => self.0.source_text.len(),
                }
            }
        };

        char_index_to_byte_offset.insert(char_index, byte_offset);
        byte_offset
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source")
            .field("id", &self.0.name)
            .field("content", &"...")
            .finish()
    }
}

/// Computes the offsets of each line in the given source string
/// and the total number of characters
fn lines_offsets(s: &str) -> (usize, Vec<usize>) {
    let mut lines = vec![0];
    let mut total = 0;

    for ch in s.chars() {
        total += 1;
        if ch == '\n' {
            lines.push(total);
        }
    }

    (total, lines)
}
