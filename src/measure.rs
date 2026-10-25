use crate::ir::ColumnIR;
use unicode_width::UnicodeWidthStr;

/// The room one badge (`PK`, `FK`, `UQ`) takes in a row, gap included.
pub const BADGE_SLOT: f64 = 22.0;
/// The badge itself, without the gap after it.
pub const BADGE_WIDTH: f64 = 18.0;

/// Where each part of a column row starts, measured from the entity's left
/// edge. Every row of an entity shares these, so names, types and flags each
/// line up in a column of their own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColumnGrid {
    /// Key badges, `PK` and `FK`
    pub key_x: f64,
    pub name_x: f64,
    pub type_x: f64,
    /// The `UQ` badge. Not null is left to the source: nearly every column of
    /// a real schema is, and a badge on each would only be noise.
    pub flag_x: f64,
    /// The width the rows need, padding on both sides included
    pub width: f64,
}

pub struct TextMetrics {
    pub char_width: f64,
    /// The advance of one character of a column row, set at 12px in a font
    /// whose glyphs are 0.6 em wide.
    pub column_char_width: f64,
    pub line_height: f64,
    pub padding_x: f64,
    pub padding_y: f64,
    pub header_padding: f64,
    pub min_node_width: f64,
    pub min_node_height: f64,
}

impl Default for TextMetrics {
    fn default() -> Self {
        Self {
            char_width: 8.0,
            column_char_width: 7.2,
            line_height: 20.0,
            padding_x: 12.0,
            padding_y: 8.0,
            header_padding: 4.0,
            min_node_width: 100.0,
            min_node_height: 60.0,
        }
    }
}

impl TextMetrics {
    pub fn text_width(&self, text: &str) -> f64 {
        let width = UnicodeWidthStr::width(text);
        width as f64 * self.char_width
    }

    /// How wide `text` is when set as part of a column row.
    pub fn column_text_width(&self, text: &str) -> f64 {
        UnicodeWidthStr::width(text) as f64 * self.column_char_width
    }

    /// How the rows of an entity are laid out across it.
    pub fn column_grid(&self, columns: &[ColumnIR]) -> ColumnGrid {
        let key_slots = columns
            .iter()
            .map(|c| c.is_pk as usize + c.is_fk as usize)
            .max()
            .unwrap_or(0);
        let flag_slots = columns
            .iter()
            .map(|c| c.is_unique as usize)
            .max()
            .unwrap_or(0);
        let widest = |f: fn(&ColumnIR) -> &str| {
            columns
                .iter()
                .map(|c| self.column_text_width(f(c)))
                .fold(0.0, f64::max)
        };
        let gap = self.column_char_width * 2.0;

        let key_x = self.padding_x;
        let name_x = key_x + key_slots as f64 * BADGE_SLOT;
        let type_x = name_x + widest(|c| &c.name) + gap;
        let type_end = type_x + widest(|c| &c.typ);
        let (flag_x, end) = if flag_slots == 0 {
            (type_end, type_end)
        } else {
            let flag_x = type_end + gap;
            (
                flag_x,
                flag_x + flag_slots as f64 * BADGE_SLOT - (BADGE_SLOT - BADGE_WIDTH),
            )
        };

        ColumnGrid {
            key_x,
            name_x,
            type_x,
            flag_x,
            width: end + self.padding_x,
        }
    }

    pub fn node_size(&self, label: &str, columns: &[ColumnIR]) -> (f64, f64) {
        let header_width = self.text_width(label) + self.padding_x * 2.0;
        let rows_width = if columns.is_empty() {
            0.0
        } else {
            self.column_grid(columns).width
        };
        let width = header_width.max(rows_width).max(self.min_node_width);

        let header_height = self.line_height + self.header_padding * 2.0;
        let body_height = if columns.is_empty() {
            0.0
        } else {
            columns.len() as f64 * self.line_height + self.padding_y * 2.0
        };

        let height = (header_height + body_height).max(self.min_node_height);

        (width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, typ: &str) -> ColumnIR {
        ColumnIR {
            name: name.to_string(),
            typ: typ.to_string(),
            is_pk: false,
            is_fk: false,
            is_unique: false,
            is_not_null: false,
        }
    }

    #[test]
    fn test_grid_lines_types_up_after_the_longest_name() {
        let m = TextMetrics::default();
        let mut id = column("id", "int");
        id.is_pk = true;
        let columns = vec![id, column("created_at", "timestamp")];
        let grid = m.column_grid(&columns);
        assert_eq!(grid.name_x, m.padding_x + BADGE_SLOT);
        assert_eq!(
            grid.type_x,
            grid.name_x + m.column_text_width("created_at") + m.column_char_width * 2.0
        );
        assert_eq!(grid.flag_x, grid.type_x + m.column_text_width("timestamp"));
    }

    #[test]
    fn test_grid_leaves_no_room_for_badges_no_row_has() {
        let m = TextMetrics::default();
        let grid = m.column_grid(&[column("name", "string")]);
        assert_eq!(grid.name_x, m.padding_x);
        assert_eq!(grid.width, grid.flag_x + m.padding_x);
    }

    #[test]
    fn test_ascii_width() {
        let m = TextMetrics::default();
        assert_eq!(m.text_width("User"), 4.0 * 8.0);
    }

    #[test]
    fn test_unicode_width() {
        let m = TextMetrics::default();
        // 全角文字は幅2
        assert_eq!(m.text_width("ユーザー"), 8.0 * 8.0);
    }

    #[test]
    fn test_mixed_width() {
        let m = TextMetrics::default();
        // "User" (4) + "テスト" (6) = 10
        assert_eq!(m.text_width("Userテスト"), 10.0 * 8.0);
    }

    #[test]
    fn test_node_size_no_columns() {
        let m = TextMetrics::default();
        let (w, h) = m.node_size("User", &[]);
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_node_size_with_columns() {
        let m = TextMetrics::default();
        let columns = vec![column("id", "int"), column("name", "string")];
        let (w, h) = m.node_size("User", &columns);
        assert!(w > 0.0);
        assert!(h > m.line_height);
    }
}
