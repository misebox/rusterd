//! SQL dialect detection and handling.

/// SQL dialect variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// Auto-detect from dump content
    #[default]
    Auto,
    /// Standard SQL
    Generic,
    /// PostgreSQL
    PostgreSQL,
    /// MySQL
    MySQL,
}

impl Dialect {
    /// Parse dialect from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "generic" => Some(Self::Generic),
            "postgres" | "postgresql" => Some(Self::PostgreSQL),
            "mysql" => Some(Self::MySQL),
            _ => None,
        }
    }

    /// Detect dialect from SQL content.
    pub fn detect(content: &str) -> Self {
        let lower = content.to_lowercase();

        // Check header comments
        if lower.contains("postgresql database dump")
            || lower.contains("pg_dump")
            || lower.contains("-- postgres")
        {
            return Self::PostgreSQL;
        }
        if lower.contains("mysql dump")
            || lower.contains("mysqldump")
            || lower.contains("-- mysql")
        {
            return Self::MySQL;
        }

        // Check type keywords
        if lower.contains("serial")
            || lower.contains("text[]")
            || lower.contains("::text")
            || lower.contains("timestamptz")
        {
            return Self::PostgreSQL;
        }
        if lower.contains("auto_increment")
            || lower.contains("tinyint")
            || lower.contains("engine=")
            || lower.contains("unsigned")
        {
            return Self::MySQL;
        }

        Self::Generic
    }

    /// Resolve Auto to a concrete dialect.
    pub fn resolve(self, content: &str) -> Self {
        match self {
            Self::Auto => Self::detect(content),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_postgres() {
        let sql = "-- PostgreSQL database dump\nCREATE TABLE users (id SERIAL);";
        assert_eq!(Dialect::detect(sql), Dialect::PostgreSQL);
    }

    #[test]
    fn test_detect_mysql() {
        let sql = "-- MySQL dump\nCREATE TABLE users (id INT AUTO_INCREMENT);";
        assert_eq!(Dialect::detect(sql), Dialect::MySQL);
    }

    #[test]
    fn test_detect_generic() {
        let sql = "CREATE TABLE users (id INTEGER PRIMARY KEY);";
        assert_eq!(Dialect::detect(sql), Dialect::Generic);
    }
}
