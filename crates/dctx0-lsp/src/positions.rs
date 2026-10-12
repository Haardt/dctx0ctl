use lsp_types::{Position, Range};

/// Converts half-open UTF-8 byte spans to the UTF-16 positions used by editors.
pub struct TextIndex<'a> {
    text: &'a str,
    lines: Vec<usize>,
}
impl<'a> TextIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut lines = vec![0];
        lines.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        Self { text, lines }
    }
    pub fn offset(&self, position: Position) -> Option<usize> {
        let start = *self.lines.get(position.line as usize)?;
        let end = self
            .lines
            .get(position.line as usize + 1)
            .copied()
            .unwrap_or(self.text.len());
        let line = self.text[start..end].trim_end_matches(['\r', '\n']);
        let mut units = 0;
        for (byte, ch) in line.char_indices() {
            if units == position.character {
                return Some(start + byte);
            }
            units += ch.len_utf16() as u32;
            if units > position.character {
                return None;
            }
        }
        (units == position.character).then_some(start + line.len())
    }
    pub fn position(&self, offset: usize) -> Position {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.lines.partition_point(|start| *start <= offset) - 1;
        Position::new(
            line as u32,
            self.text[self.lines[line]..offset]
                .trim_end_matches('\r')
                .encode_utf16()
                .count() as u32,
        )
    }
    pub fn range(&self, start: usize, end: usize) -> Range {
        Range::new(self.position(start), self.position(end))
    }
}
