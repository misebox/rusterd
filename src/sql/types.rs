//! SQL to ERD type mapping.

use super::Dialect;

/// Map SQL type to ERD type.
pub fn map_type(sql_type: &str, dialect: Dialect) -> String {
    let lower = sql_type.to_lowercase();
    let base = lower.split('(').next().unwrap_or(&lower).trim();

    match dialect {
        Dialect::PostgreSQL => map_postgres_type(base),
        Dialect::MySQL => map_mysql_type(base, &lower),
        _ => map_generic_type(base),
    }
}

fn map_postgres_type(base: &str) -> String {
    match base {
        // Integer types
        "int" | "int4" | "integer" | "serial" | "serial4" => "int".to_string(),
        "bigint" | "int8" | "bigserial" | "serial8" => "bigint".to_string(),
        "smallint" | "int2" | "smallserial" | "serial2" => "smallint".to_string(),

        // Floating point
        "real" | "float4" => "float".to_string(),
        "double precision" | "float8" => "double".to_string(),
        "decimal" | "numeric" => "decimal".to_string(),

        // String types
        "varchar" | "character varying" => "varchar".to_string(),
        "char" | "character" => "char".to_string(),
        "text" => "text".to_string(),

        // Date/time
        "timestamp" | "timestamptz" | "timestamp with time zone"
        | "timestamp without time zone" => "timestamp".to_string(),
        "date" => "date".to_string(),
        "time" | "timetz" => "time".to_string(),
        "interval" => "interval".to_string(),

        // Boolean
        "boolean" | "bool" => "boolean".to_string(),

        // Binary
        "bytea" => "bytea".to_string(),

        // UUID
        "uuid" => "uuid".to_string(),

        // JSON
        "json" | "jsonb" => "json".to_string(),

        // Arrays (strip array notation)
        t if t.ends_with("[]") => {
            let inner = &t[..t.len() - 2];
            format!("{}[]", map_postgres_type(inner))
        }

        // Default: keep original
        _ => base.to_string(),
    }
}

fn map_mysql_type(base: &str, full: &str) -> String {
    match base {
        // Integer types
        "int" | "integer" => "int".to_string(),
        "bigint" => "bigint".to_string(),
        "smallint" => "smallint".to_string(),
        "mediumint" => "mediumint".to_string(),
        "tinyint" => {
            // TINYINT(1) is often used as boolean
            if full.contains("tinyint(1)") {
                "boolean".to_string()
            } else {
                "tinyint".to_string()
            }
        }

        // Floating point
        "float" => "float".to_string(),
        "double" => "double".to_string(),
        "decimal" | "numeric" => "decimal".to_string(),

        // String types
        "varchar" => "varchar".to_string(),
        "char" => "char".to_string(),
        "text" | "longtext" | "mediumtext" | "tinytext" => "text".to_string(),

        // Date/time
        "datetime" | "timestamp" => "timestamp".to_string(),
        "date" => "date".to_string(),
        "time" => "time".to_string(),
        "year" => "year".to_string(),

        // Binary
        "blob" | "longblob" | "mediumblob" | "tinyblob" => "blob".to_string(),
        "binary" | "varbinary" => "binary".to_string(),

        // JSON
        "json" => "json".to_string(),

        // Enum/Set
        "enum" | "set" => "enum".to_string(),

        // Default
        _ => base.to_string(),
    }
}

fn map_generic_type(base: &str) -> String {
    match base {
        "int" | "integer" => "int".to_string(),
        "bigint" => "bigint".to_string(),
        "smallint" => "smallint".to_string(),
        "real" | "float" => "float".to_string(),
        "double" | "double precision" => "double".to_string(),
        "decimal" | "numeric" => "decimal".to_string(),
        "varchar" | "character varying" => "varchar".to_string(),
        "char" | "character" => "char".to_string(),
        "text" => "text".to_string(),
        "timestamp" | "datetime" => "timestamp".to_string(),
        "date" => "date".to_string(),
        "time" => "time".to_string(),
        "boolean" | "bool" => "boolean".to_string(),
        "blob" => "blob".to_string(),
        _ => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postgres_types() {
        assert_eq!(map_type("SERIAL", Dialect::PostgreSQL), "int");
        assert_eq!(map_type("VARCHAR(255)", Dialect::PostgreSQL), "varchar");
        assert_eq!(map_type("TIMESTAMPTZ", Dialect::PostgreSQL), "timestamp");
        assert_eq!(map_type("JSONB", Dialect::PostgreSQL), "json");
    }

    #[test]
    fn test_mysql_types() {
        assert_eq!(map_type("INT", Dialect::MySQL), "int");
        assert_eq!(map_type("TINYINT(1)", Dialect::MySQL), "boolean");
        assert_eq!(map_type("TINYINT(4)", Dialect::MySQL), "tinyint");
        assert_eq!(map_type("DATETIME", Dialect::MySQL), "timestamp");
    }
}
