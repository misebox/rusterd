use unicode_width::UnicodeWidthStr;

pub struct TextMetrics {
    pub char_width: f64,
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

    pub fn node_size(&self, label: &str, columns: &[(String, String)]) -> (f64, f64) {
        let header_width = self.text_width(label);

        let max_col_width = columns
            .iter()
            .map(|(name, typ)| self.text_width(name) + self.text_width(typ) + self.char_width * 2.0)
            .fold(0.0, f64::max);

        let content_width = header_width.max(max_col_width) + self.padding_x * 2.0;
        let width = content_width.max(self.min_node_width);

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
        let columns = vec![
            ("id".to_string(), "int".to_string()),
            ("name".to_string(), "string".to_string()),
        ];
        let (w, h) = m.node_size("User", &columns);
        assert!(w > 0.0);
        assert!(h > m.line_height);
    }
}
