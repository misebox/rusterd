//! SQL parser for CREATE TABLE statements.

use super::dialect::Dialect;
use super::lexer::{Lexer, Token};
use super::types::map_type;
use crate::ast::{
    Cardinality, Column, ColumnModifier, Constraint, Entity, Relationship, Schema,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SqlParseError {
    #[error("Unexpected token: {0:?}")]
    UnexpectedToken(Token),
    #[error("Expected {expected}, found {found:?}")]
    Expected { expected: String, found: Token },
    #[error("Unexpected end of input")]
    UnexpectedEof,
}

/// Parse SQL dump to Schema.
pub fn parse_sql(input: &str, dialect: Dialect) -> Result<Schema, SqlParseError> {
    let dialect = dialect.resolve(input);
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens, dialect);
    parser.parse()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    dialect: Dialect,
}

impl Parser {
    fn new(tokens: Vec<Token>, dialect: Dialect) -> Self {
        Self {
            tokens,
            pos: 0,
            dialect,
        }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn parse(&mut self) -> Result<Schema, SqlParseError> {
        let mut entities = Vec::new();
        let mut fk_constraints: Vec<(String, FkInfo)> = Vec::new();

        while self.current() != &Token::Eof {
            match self.current() {
                Token::Create => {
                    self.advance();

                    // Skip IF NOT EXISTS
                    if self.current() == &Token::If {
                        self.skip_until_token(&Token::Table);
                    }

                    if self.current() == &Token::Table {
                        self.advance();

                        // Skip IF NOT EXISTS after TABLE
                        if self.current() == &Token::If {
                            self.advance(); // IF
                            if self.current() == &Token::Not {
                                self.advance(); // NOT
                            }
                            if self.current() == &Token::Exists {
                                self.advance(); // EXISTS
                            }
                        }

                        if let Some((entity, fks)) = self.parse_create_table()? {
                            let table_name = entity.name.clone();
                            entities.push(entity);
                            for fk in fks {
                                fk_constraints.push((table_name.clone(), fk));
                            }
                        }
                    } else {
                        // Skip other CREATE statements (INDEX, VIEW, etc.)
                        self.skip_statement();
                    }
                }
                Token::Alter => {
                    // ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY
                    if let Some((table_name, fk)) = self.parse_alter_table_fk()? {
                        fk_constraints.push((table_name, fk));
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }

        // Generate relationships from FK constraints
        let relationships = self.generate_relationships(&entities, &fk_constraints);

        Ok(Schema {
            entities,
            relationships,
            views: vec![],
            arrangement: None,
        })
    }

    fn parse_create_table(&mut self) -> Result<Option<(Entity, Vec<FkInfo>)>, SqlParseError> {
        // Table name
        let table_name = match self.current() {
            Token::Ident(name) => name.clone(),
            _ => {
                self.skip_statement();
                return Ok(None);
            }
        };
        self.advance();

        // Handle schema.table format
        if self.current() == &Token::Dot {
            self.advance();
            // Use the second part as table name
            let _schema_name = table_name;
            let table_name = match self.current() {
                Token::Ident(name) => name.clone(),
                _ => {
                    self.skip_statement();
                    return Ok(None);
                }
            };
            self.advance();
            return self.parse_table_body(table_name);
        }

        self.parse_table_body(table_name)
    }

    fn parse_table_body(
        &mut self,
        table_name: String,
    ) -> Result<Option<(Entity, Vec<FkInfo>)>, SqlParseError> {
        if self.current() != &Token::LParen {
            self.skip_statement();
            return Ok(None);
        }
        self.advance();

        let mut columns = Vec::new();
        let mut constraints = Vec::new();
        let mut fk_infos = Vec::new();
        let mut pk_columns: Vec<String> = Vec::new();

        loop {
            match self.current() {
                Token::RParen => {
                    self.advance();
                    break;
                }
                Token::Comma => {
                    self.advance();
                }
                Token::Primary => {
                    // PRIMARY KEY (col1, col2, ...)
                    self.advance();
                    if self.current() == &Token::Key {
                        self.advance();
                        let cols = self.parse_column_list()?;
                        pk_columns.extend(cols.clone());
                        if cols.len() > 1 {
                            constraints.push(Constraint::PrimaryKey(cols));
                        }
                    }
                }
                Token::Foreign => {
                    // FOREIGN KEY (col) REFERENCES table(col)
                    if let Some(fk) = self.parse_foreign_key_constraint()? {
                        fk_infos.push(fk);
                    }
                }
                Token::Unique => {
                    // UNIQUE (col1, col2, ...)
                    self.advance();
                    if self.current() == &Token::Key {
                        self.advance();
                    }
                    if self.current() == &Token::LParen {
                        let _cols = self.parse_column_list()?;
                        // Skip UNIQUE constraint for now
                    }
                }
                Token::Constraint => {
                    // Named constraint
                    self.advance();
                    // Skip constraint name
                    if let Token::Ident(_) = self.current() {
                        self.advance();
                    }
                    // Continue to parse the constraint type
                }
                Token::Index | Token::Key => {
                    // INDEX or KEY definition
                    self.skip_until(&[Token::Comma, Token::RParen]);
                }
                Token::Check => {
                    // CHECK constraint
                    self.skip_parenthesized();
                }
                Token::Ident(_) => {
                    // Column definition
                    if let Some(col) = self.parse_column()? {
                        columns.push(col);
                    }
                }
                Token::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }

        // Skip table options (ENGINE=, etc.)
        self.skip_statement();

        // Apply PK modifier to columns
        for col in &mut columns {
            if pk_columns.contains(&col.name) {
                if !col.modifiers.iter().any(|m| matches!(m, ColumnModifier::Pk)) {
                    col.modifiers.insert(0, ColumnModifier::Pk);
                }
            }
        }

        Ok(Some((
            Entity {
                name: table_name,
                columns,
                constraints,
                hints: vec![],
            },
            fk_infos,
        )))
    }

    fn parse_column(&mut self) -> Result<Option<Column>, SqlParseError> {
        let name = match self.current() {
            Token::Ident(n) => n.clone(),
            _ => return Ok(None),
        };
        self.advance();

        // Type
        let mut type_parts = Vec::new();
        let mut paren_depth = 0;

        loop {
            match self.current() {
                Token::Ident(t) => {
                    type_parts.push(t.clone());
                    self.advance();
                }
                Token::Serial => {
                    type_parts.push("SERIAL".to_string());
                    self.advance();
                }
                Token::LParen => {
                    paren_depth += 1;
                    type_parts.push("(".to_string());
                    self.advance();
                }
                Token::RParen if paren_depth > 0 => {
                    paren_depth -= 1;
                    type_parts.push(")".to_string());
                    self.advance();
                }
                Token::Num(n) => {
                    type_parts.push(n.clone());
                    self.advance();
                }
                Token::Comma if paren_depth > 0 => {
                    type_parts.push(",".to_string());
                    self.advance();
                }
                _ => break,
            }
        }

        if type_parts.is_empty() {
            return Ok(None);
        }

        let raw_type = type_parts.join("");
        let typ = map_type(&raw_type, self.dialect);

        // Parse modifiers
        let mut modifiers = Vec::new();
        let mut is_pk = false;

        loop {
            match self.current() {
                Token::Primary => {
                    self.advance();
                    if self.current() == &Token::Key {
                        self.advance();
                    }
                    is_pk = true;
                }
                Token::Not => {
                    self.advance();
                    if self.current() == &Token::Null {
                        self.advance();
                        modifiers.push(ColumnModifier::NotNull);
                    }
                }
                Token::Null => {
                    self.advance();
                    // Nullable (no modifier needed)
                }
                Token::Unique => {
                    self.advance();
                    if self.current() == &Token::Key {
                        self.advance();
                    }
                    modifiers.push(ColumnModifier::Unique);
                }
                Token::Default => {
                    self.advance();
                    let default_val = self.parse_default_value()?;
                    modifiers.push(ColumnModifier::Default(default_val));
                }
                Token::References => {
                    // Inline FK reference
                    self.advance();
                    let (target, col) = self.parse_reference()?;
                    modifiers.push(ColumnModifier::Fk { target, column: col });
                    // Skip ON DELETE/UPDATE
                    self.skip_on_actions();
                }
                Token::Increment | Token::Auto => {
                    self.advance();
                    // AUTO_INCREMENT implies PK usually, but don't force it
                    if self.current() == &Token::Increment {
                        self.advance();
                    }
                }
                Token::Serial => {
                    self.advance();
                    // SERIAL implies PK
                }
                Token::Check => {
                    self.skip_parenthesized();
                }
                Token::Comma | Token::RParen | Token::Eof => break,
                Token::Constraint => {
                    // Inline constraint
                    self.advance();
                    if let Token::Ident(_) = self.current() {
                        self.advance();
                    }
                }
                Token::On => {
                    // ON DELETE/UPDATE for inline FK
                    self.skip_on_actions();
                }
                _ => {
                    self.advance();
                }
            }
        }

        if is_pk {
            modifiers.insert(0, ColumnModifier::Pk);
        }

        Ok(Some(Column {
            name,
            typ,
            modifiers,
        }))
    }

    fn parse_default_value(&mut self) -> Result<String, SqlParseError> {
        match self.current() {
            Token::Str(s) => {
                let val = s.clone();
                self.advance();
                Ok(val)
            }
            Token::Num(n) => {
                let val = n.clone();
                self.advance();
                Ok(val)
            }
            Token::Null => {
                self.advance();
                Ok("NULL".to_string())
            }
            Token::Ident(s) => {
                let mut val = s.clone();
                self.advance();
                // Handle function calls like NOW()
                if self.current() == &Token::LParen {
                    val.push('(');
                    self.advance();
                    if self.current() == &Token::RParen {
                        val.push(')');
                        self.advance();
                    } else {
                        // Nested content
                        let inner = self.collect_until_paren()?;
                        val.push_str(&inner);
                        val.push(')');
                    }
                }
                Ok(val)
            }
            Token::LParen => {
                // Expression in parentheses
                self.advance();
                let inner = self.collect_until_paren()?;
                Ok(format!("({})", inner))
            }
            _ => Ok(String::new()),
        }
    }

    fn collect_until_paren(&mut self) -> Result<String, SqlParseError> {
        let mut parts = Vec::new();
        let mut depth = 1;

        loop {
            match self.current() {
                Token::LParen => {
                    depth += 1;
                    parts.push("(".to_string());
                    self.advance();
                }
                Token::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        self.advance();
                        break;
                    }
                    parts.push(")".to_string());
                    self.advance();
                }
                Token::Ident(s) => {
                    parts.push(s.clone());
                    self.advance();
                }
                Token::Num(n) => {
                    parts.push(n.clone());
                    self.advance();
                }
                Token::Str(s) => {
                    parts.push(format!("'{}'", s));
                    self.advance();
                }
                Token::Comma => {
                    parts.push(",".to_string());
                    self.advance();
                }
                Token::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }

        Ok(parts.join(" "))
    }

    fn parse_reference(&mut self) -> Result<(String, String), SqlParseError> {
        let target = match self.current() {
            Token::Ident(t) => t.clone(),
            _ => return Err(SqlParseError::UnexpectedToken(self.current().clone())),
        };
        self.advance();

        // Handle schema.table
        let target = if self.current() == &Token::Dot {
            self.advance();
            match self.current() {
                Token::Ident(t) => {
                    let t = t.clone();
                    self.advance();
                    t
                }
                _ => target,
            }
        } else {
            target
        };

        // (column)
        let col = if self.current() == &Token::LParen {
            self.advance();
            let col = match self.current() {
                Token::Ident(c) => c.clone(),
                _ => "id".to_string(),
            };
            self.advance();
            if self.current() == &Token::RParen {
                self.advance();
            }
            col
        } else {
            "id".to_string()
        };

        Ok((target, col))
    }

    fn parse_foreign_key_constraint(&mut self) -> Result<Option<FkInfo>, SqlParseError> {
        self.advance(); // FOREIGN
        if self.current() != &Token::Key {
            return Ok(None);
        }
        self.advance(); // KEY

        // (columns)
        let columns = self.parse_column_list()?;

        if self.current() != &Token::References {
            return Ok(None);
        }
        self.advance();

        let (target, target_col) = self.parse_reference()?;

        // ON DELETE/UPDATE
        self.skip_on_actions();

        Ok(Some(FkInfo {
            columns,
            target,
            target_column: target_col,
        }))
    }

    fn parse_column_list(&mut self) -> Result<Vec<String>, SqlParseError> {
        let mut cols = Vec::new();

        if self.current() != &Token::LParen {
            return Ok(cols);
        }
        self.advance();

        loop {
            match self.current() {
                Token::Ident(name) => {
                    cols.push(name.clone());
                    self.advance();
                }
                Token::Comma => {
                    self.advance();
                }
                Token::RParen => {
                    self.advance();
                    break;
                }
                Token::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }

        Ok(cols)
    }

    fn skip_on_actions(&mut self) {
        while self.current() == &Token::On {
            self.advance();
            // DELETE or UPDATE
            if matches!(self.current(), Token::Delete | Token::Update) {
                self.advance();
            }
            // Action: CASCADE, RESTRICT, SET NULL, SET DEFAULT, NO ACTION
            match self.current() {
                Token::Cascade | Token::Restrict => {
                    self.advance();
                }
                Token::Ident(s) if s.to_uppercase() == "SET" => {
                    self.advance();
                    if matches!(self.current(), Token::Null | Token::Default) {
                        self.advance();
                    }
                }
                Token::Ident(s) if s.to_uppercase() == "NO" => {
                    self.advance();
                    if let Token::Ident(a) = self.current() {
                        if a.to_uppercase() == "ACTION" {
                            self.advance();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn skip_parenthesized(&mut self) {
        if self.current() != &Token::LParen {
            self.advance();
            return;
        }
        self.advance();
        let mut depth = 1;
        while depth > 0 {
            match self.current() {
                Token::LParen => {
                    depth += 1;
                    self.advance();
                }
                Token::RParen => {
                    depth -= 1;
                    self.advance();
                }
                Token::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn skip_statement(&mut self) {
        while !matches!(self.current(), Token::Semicolon | Token::Eof) {
            self.advance();
        }
        if self.current() == &Token::Semicolon {
            self.advance();
        }
    }

    fn skip_until(&mut self, tokens: &[Token]) {
        while !tokens.contains(self.current()) && self.current() != &Token::Eof {
            if self.current() == &Token::LParen {
                self.skip_parenthesized();
            } else {
                self.advance();
            }
        }
    }

    fn skip_until_token(&mut self, token: &Token) {
        while self.current() != token && self.current() != &Token::Eof {
            self.advance();
        }
    }

    fn generate_relationships(
        &self,
        entities: &[Entity],
        fk_constraints: &[(String, FkInfo)],
    ) -> Vec<Relationship> {
        let entity_names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let mut relationships = Vec::new();

        for (source_table, fk) in fk_constraints {
            // Check if target table exists
            if !entity_names.contains(&fk.target.as_str()) {
                continue;
            }

            relationships.push(Relationship {
                left: fk.target.clone(),
                left_cardinality: Cardinality::One,
                right: source_table.clone(),
                right_cardinality: Cardinality::Many,
                label: None,
                role: None,
            });
        }

        // Also check inline FK modifiers on columns
        for entity in entities {
            for col in &entity.columns {
                for modifier in &col.modifiers {
                    if let ColumnModifier::Fk { target, .. } = modifier {
                        if entity_names.contains(&target.as_str()) {
                            // Avoid duplicates
                            let exists = relationships.iter().any(|r| {
                                r.left == *target && r.right == entity.name
                            });
                            if !exists {
                                relationships.push(Relationship {
                                    left: target.clone(),
                                    left_cardinality: Cardinality::One,
                                    right: entity.name.clone(),
                                    right_cardinality: Cardinality::Many,
                                    label: None,
                                    role: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        relationships
    }

    /// Parse ALTER TABLE ... ADD CONSTRAINT ... FOREIGN KEY
    fn parse_alter_table_fk(&mut self) -> Result<Option<(String, FkInfo)>, SqlParseError> {
        self.advance(); // ALTER

        if self.current() != &Token::Table {
            self.skip_statement();
            return Ok(None);
        }
        self.advance(); // TABLE

        // Skip ONLY if present
        if self.current() == &Token::Only {
            self.advance();
        }

        // Get table name (possibly schema.table)
        let table_name = match self.current() {
            Token::Ident(name) => name.clone(),
            _ => {
                self.skip_statement();
                return Ok(None);
            }
        };
        self.advance();

        // Handle schema.table format
        let table_name = if self.current() == &Token::Dot {
            self.advance();
            match self.current() {
                Token::Ident(name) => {
                    let name = name.clone();
                    self.advance();
                    name
                }
                _ => {
                    self.skip_statement();
                    return Ok(None);
                }
            }
        } else {
            table_name
        };

        // Look for ADD CONSTRAINT ... FOREIGN KEY
        if self.current() != &Token::Add {
            self.skip_statement();
            return Ok(None);
        }
        self.advance(); // ADD

        if self.current() != &Token::Constraint {
            self.skip_statement();
            return Ok(None);
        }
        self.advance(); // CONSTRAINT

        // Skip constraint name
        if let Token::Ident(_) = self.current() {
            self.advance();
        }

        // Check for FOREIGN KEY
        if self.current() != &Token::Foreign {
            self.skip_statement();
            return Ok(None);
        }

        // Parse the FK constraint
        if let Some(fk) = self.parse_foreign_key_constraint()? {
            Ok(Some((table_name, fk)))
        } else {
            self.skip_statement();
            Ok(None)
        }
    }
}

struct FkInfo {
    #[allow(dead_code)]
    columns: Vec<String>,
    target: String,
    #[allow(dead_code)]
    target_column: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_table() {
        let sql = r#"
            CREATE TABLE users (
                id INT PRIMARY KEY,
                email VARCHAR(255) NOT NULL UNIQUE
            );
        "#;

        let schema = parse_sql(sql, Dialect::Generic).unwrap();
        assert_eq!(schema.entities.len(), 1);

        let user = &schema.entities[0];
        assert_eq!(user.name, "users");
        assert_eq!(user.columns.len(), 2);

        assert_eq!(user.columns[0].name, "id");
        assert!(user.columns[0]
            .modifiers
            .iter()
            .any(|m| matches!(m, ColumnModifier::Pk)));

        assert_eq!(user.columns[1].name, "email");
        assert!(user.columns[1]
            .modifiers
            .iter()
            .any(|m| matches!(m, ColumnModifier::NotNull)));
        assert!(user.columns[1]
            .modifiers
            .iter()
            .any(|m| matches!(m, ColumnModifier::Unique)));
    }

    #[test]
    fn test_parse_with_foreign_key() {
        let sql = r#"
            CREATE TABLE users (id INT PRIMARY KEY);
            CREATE TABLE orders (
                id INT PRIMARY KEY,
                user_id INT REFERENCES users(id)
            );
        "#;

        let schema = parse_sql(sql, Dialect::Generic).unwrap();
        assert_eq!(schema.entities.len(), 2);
        assert_eq!(schema.relationships.len(), 1);

        let rel = &schema.relationships[0];
        assert_eq!(rel.left, "users");
        assert_eq!(rel.right, "orders");
    }

    #[test]
    fn test_parse_postgres_serial() {
        let sql = r#"
            CREATE TABLE users (
                id SERIAL PRIMARY KEY,
                name TEXT
            );
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();
        let user = &schema.entities[0];

        assert_eq!(user.columns[0].typ, "int");
    }

    #[test]
    fn test_parse_mysql_auto_increment() {
        let sql = r#"
            CREATE TABLE users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(255)
            ) ENGINE=InnoDB;
        "#;

        let schema = parse_sql(sql, Dialect::MySQL).unwrap();
        let user = &schema.entities[0];

        assert_eq!(user.columns[0].name, "id");
        assert!(user.columns[0]
            .modifiers
            .iter()
            .any(|m| matches!(m, ColumnModifier::Pk)));
    }
}
