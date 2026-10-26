//! SQL parser for CREATE TABLE statements.

use super::dialect::Dialect;
use super::lexer::{Lexer, Token};
use super::types::map_type;
use crate::ast::{Cardinality, Column, ColumnModifier, Constraint, Entity, Relationship, Schema};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SqlParseError {
    #[error("Unexpected token: {0:?}")]
    UnexpectedToken(Token),
    #[error("Expected {expected}, found {found:?}")]
    Expected { expected: String, found: Token },
    #[error("Unexpected end of input")]
    UnexpectedEof,
    #[error("Unbalanced parentheses in the statement on table `{0}`")]
    UnbalancedParens(String),
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
    /// Tokens from here on read as end of input
    end: usize,
    dialect: Dialect,
}

impl Parser {
    fn new(tokens: Vec<Token>, dialect: Dialect) -> Self {
        Self {
            end: tokens.len(),
            tokens,
            pos: 0,
            dialect,
        }
    }

    fn current(&self) -> &Token {
        if self.pos < self.end {
            &self.tokens[self.pos]
        } else {
            &Token::Eof
        }
    }

    fn advance(&mut self) {
        if self.pos < self.end {
            self.pos += 1;
        }
    }

    /// Run `f` seeing only the tokens before `end`, then resume after `end`.
    /// Whatever `f` fails to understand cannot reach past `end`.
    fn within<T>(&mut self, end: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let outer = self.end;
        self.end = end;
        let result = f(self);
        self.end = outer;
        self.pos = end + 1;
        result
    }

    /// Index of the first of `stops` outside any parentheses, or of the
    /// parenthesis closing the current nesting; `end` when there is neither.
    fn boundary(&self, stops: &[Token]) -> usize {
        let mut depth = 0;
        for i in self.pos..self.end {
            match &self.tokens[i] {
                Token::LParen => depth += 1,
                Token::RParen if depth == 0 => return i,
                Token::RParen => depth -= 1,
                t if depth == 0 && stops.contains(t) => return i,
                _ => {}
            }
        }
        self.end
    }

    fn parse(&mut self) -> Result<Schema, SqlParseError> {
        let mut entities = Vec::new();
        let mut fk_constraints: Vec<(String, FkInfo)> = Vec::new();

        while self.pos < self.tokens.len() {
            let end = self.tokens[self.pos..]
                .iter()
                .position(|t| t == &Token::Semicolon)
                .map_or(self.tokens.len(), |i| self.pos + i);
            self.within(end, |p| {
                p.parse_statement(&mut entities, &mut fk_constraints)
            })?;
        }

        // Generate relationships from FK constraints
        let relationships = self.generate_relationships(&entities, &fk_constraints);

        Ok(Schema {
            entities,
            relationships,
            focuses: vec![],
            near: Vec::new(),
            omit: Vec::new(),
            brief: Vec::new(),
        })
    }

    fn parse_statement(
        &mut self,
        entities: &mut Vec<Entity>,
        fk_constraints: &mut Vec<(String, FkInfo)>,
    ) -> Result<(), SqlParseError> {
        if self.eat_word("DROP") {
            if self.current() == &Token::Table {
                self.advance();
                self.parse_drop_table(entities, fk_constraints);
            }
            return Ok(());
        }

        self.skip_until(&[Token::Create, Token::Alter]);

        match self.current() {
            Token::Create => {
                self.advance();

                // CREATE [TEMPORARY | UNLOGGED | ...] TABLE
                while TABLE_KINDS.iter().any(|kind| self.is_word(kind)) {
                    self.advance();
                }

                // Skip IF NOT EXISTS
                if self.current() == &Token::If {
                    self.skip_until_token(&Token::Table);
                }

                // Other CREATE statements (INDEX, VIEW, etc.) add nothing
                if self.current() != &Token::Table {
                    return Ok(());
                }
                self.advance();

                self.skip_if_not_exists();

                if let Some((entity, fks)) = self.parse_create_table()? {
                    let table_name = entity.name.clone();
                    entities.push(entity);
                    for fk in fks {
                        fk_constraints.push((table_name.clone(), fk));
                    }
                }
            }
            Token::Alter => {
                let Some((table_name, alterations)) = self.parse_alter_table()? else {
                    return Ok(());
                };
                for alteration in alterations {
                    alter(entities, fk_constraints, &table_name, alteration);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Parse the rest of DROP TABLE [IF EXISTS] name, ...
    fn parse_drop_table(
        &mut self,
        entities: &mut Vec<Entity>,
        fk_constraints: &mut Vec<(String, FkInfo)>,
    ) {
        self.skip_if_not_exists();
        while let Some(name) = self.parse_table_name() {
            entities.retain(|e| e.name != name);
            fk_constraints.retain(|(source, _)| source != &name);
            if self.current() == &Token::Comma {
                self.advance();
            }
        }
    }

    /// Parse [schema.]table, keeping the table.
    fn parse_table_name(&mut self) -> Option<String> {
        let name = self.eat_ident()?;
        if self.current() != &Token::Dot {
            return Some(name);
        }
        self.advance();
        self.eat_ident()
    }

    /// Fail when the parentheses in the rest of the statement do not pair up.
    fn check_parens(&self, table: &str) -> Result<(), SqlParseError> {
        let mut depth = 0;
        for token in &self.tokens[self.pos..self.end] {
            match token {
                Token::LParen => depth += 1,
                Token::RParen => depth -= 1,
                _ => {}
            }
            if depth < 0 {
                break;
            }
        }
        if depth != 0 {
            return Err(SqlParseError::UnbalancedParens(table.to_string()));
        }
        Ok(())
    }

    fn parse_create_table(&mut self) -> Result<Option<(Entity, Vec<FkInfo>)>, SqlParseError> {
        let Some(table_name) = self.parse_table_name() else {
            return Ok(None);
        };
        self.check_parens(&table_name)?;

        // Without a body (AS SELECT, PARTITION OF) there are no columns to read
        if self.current() != &Token::LParen {
            return Ok(None);
        }
        self.advance();

        let mut entity = Entity {
            name: table_name,
            note: None,
            columns: Vec::new(),
            constraints: Vec::new(),
            hints: vec![],
        };
        let mut fk_infos = Vec::new();
        let mut primary_keys = Vec::new();

        // Items are split on the commas between them before any is read
        loop {
            let end = self.boundary(&[Token::Comma]);
            let last = self.tokens.get(end) != Some(&Token::Comma);

            match self.within(end, Self::parse_table_item)? {
                Some(TableItem::Column(col)) => entity.columns.push(col),
                Some(TableItem::PrimaryKey(cols)) => primary_keys.push(cols),
                Some(TableItem::ForeignKey(fk)) => fk_infos.push(fk),
                None => {}
            }

            if last {
                break;
            }
        }

        // Table options start with a word, so a parenthesis here closed the body early
        if self.current() == &Token::LParen {
            return Err(SqlParseError::UnexpectedToken(Token::LParen));
        }

        for cols in primary_keys {
            add_primary_key(&mut entity, cols);
        }
        for fk in &fk_infos {
            add_foreign_key(&mut entity, fk);
        }

        Ok(Some((entity, fk_infos)))
    }

    /// Parse PRIMARY KEY (col1, col2, ...)
    fn parse_primary_key(&mut self) -> Result<Vec<String>, SqlParseError> {
        self.advance(); // PRIMARY
        if self.current() == &Token::Key {
            self.advance();
        }
        self.parse_column_list()
    }

    fn is_word(&self, word: &str) -> bool {
        matches!(self.current(), Token::Ident(s) if s.eq_ignore_ascii_case(word))
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let found = self.is_word(word);
        if found {
            self.advance();
        }
        found
    }

    fn eat_ident(&mut self) -> Option<String> {
        let Token::Ident(name) = self.current() else {
            return None;
        };
        let name = name.clone();
        self.advance();
        Some(name)
    }

    /// Parse one comma-separated item of a table body.
    fn parse_table_item(&mut self) -> Result<Option<TableItem>, SqlParseError> {
        self.skip_constraint_name();

        Ok(match self.current() {
            Token::Primary => Some(TableItem::PrimaryKey(self.parse_primary_key()?)),
            // FOREIGN KEY (col) REFERENCES table(col)
            Token::Foreign => self
                .parse_foreign_key_constraint()?
                .map(TableItem::ForeignKey),
            // EXCLUDE USING ..., LIKE other_table
            Token::Ident(word) if ["EXCLUDE", "LIKE"].contains(&word.as_str()) => None,
            Token::Ident(_) => self.parse_column()?.map(TableItem::Column),
            // UNIQUE, CHECK, INDEX and anything unknown add nothing
            _ => None,
        })
    }

    /// Skip CONSTRAINT name
    fn skip_constraint_name(&mut self) {
        if self.current() != &Token::Constraint {
            return;
        }
        self.advance();
        if let Token::Ident(_) = self.current() {
            self.advance();
        }
    }

    /// Parse a column type into its ERD type.
    fn parse_type(&mut self) -> Option<String> {
        let mut raw_type = String::new();
        let mut paren_depth = 0;

        loop {
            match self.current() {
                Token::Ident(t) if paren_depth == 0 && ends_type(t) => break,
                // integer[], integer ARRAY
                Token::Array => {
                    raw_type.push_str("[]");
                    self.advance();
                }
                Token::Ident(t) if !raw_type.is_empty() && t.eq_ignore_ascii_case("ARRAY") => {
                    raw_type.push_str("[]");
                    self.advance();
                }
                Token::Ident(t) => {
                    // Words of one type stay apart: "timestamp with time zone"
                    if raw_type.ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                        raw_type.push(' ');
                    }
                    raw_type.push_str(t);
                    self.advance();
                }
                Token::Serial => {
                    raw_type.push_str("SERIAL");
                    self.advance();
                }
                // Arguments follow a type name
                Token::LParen if !raw_type.is_empty() => {
                    paren_depth += 1;
                    raw_type.push('(');
                    self.advance();
                }
                Token::RParen if paren_depth > 0 => {
                    paren_depth -= 1;
                    raw_type.push(')');
                    self.advance();
                }
                Token::Num(n) => {
                    raw_type.push_str(n);
                    self.advance();
                }
                // ENUM('a', 'b')
                Token::Str(s) if paren_depth > 0 => {
                    raw_type.push_str(&format!("'{}'", s));
                    self.advance();
                }
                Token::Comma if paren_depth > 0 => {
                    raw_type.push(',');
                    self.advance();
                }
                _ => break,
            }
        }

        if raw_type.is_empty() {
            return None;
        }
        Some(map_type(&raw_type, self.dialect))
    }

    fn parse_column(&mut self) -> Result<Option<Column>, SqlParseError> {
        let Some(name) = self.eat_ident() else {
            return Ok(None);
        };
        let Some(typ) = self.parse_type() else {
            return Ok(None);
        };

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
                    modifiers.push(ColumnModifier::Fk {
                        target,
                        column: col,
                    });
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
                    self.advance();
                    self.skip_parenthesized();
                }
                // Options in parentheses: IDENTITY (...), AS (expr)
                Token::LParen => {
                    self.skip_parenthesized();
                }
                Token::Comma | Token::RParen | Token::Semicolon | Token::Eof => break,
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
            note: None,
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
                    if let Token::Ident(a) = self.current()
                        && a.to_uppercase() == "ACTION"
                    {
                        self.advance();
                    }
                }
                _ => {}
            }
        }
    }

    fn skip_parenthesized(&mut self) {
        if self.current() != &Token::LParen {
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
                    if let ColumnModifier::Fk { target, .. } = modifier
                        && entity_names.contains(&target.as_str())
                    {
                        // Avoid duplicates
                        let exists = relationships
                            .iter()
                            .any(|r| r.left == *target && r.right == entity.name);
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

        relationships
    }

    /// Parse ALTER TABLE [IF EXISTS] [ONLY] name action, ...
    fn parse_alter_table(&mut self) -> Result<Option<(String, Vec<Alteration>)>, SqlParseError> {
        self.advance(); // ALTER

        if self.current() != &Token::Table {
            return Ok(None);
        }
        self.advance(); // TABLE

        self.skip_if_not_exists();
        if self.current() == &Token::Only {
            self.advance();
        }

        let Some(table_name) = self.parse_table_name() else {
            return Ok(None);
        };
        self.check_parens(&table_name)?;

        // Actions are split on the commas between them before any is read
        let mut alterations = Vec::new();
        while self.current() != &Token::Eof {
            let end = self.boundary(&[Token::Comma]);
            if let Some(alteration) = self.within(end, Self::parse_alteration)? {
                alterations.push(alteration);
            }
        }

        Ok(Some((table_name, alterations)))
    }

    /// Parse one comma-separated action of ALTER TABLE.
    fn parse_alteration(&mut self) -> Result<Option<Alteration>, SqlParseError> {
        if self.current() == &Token::Add {
            self.advance();
            return self.parse_add();
        }
        if self.current() == &Token::Alter {
            self.advance();
            return self.parse_alter_column();
        }
        if self.eat_word("DROP") {
            return Ok(self.parse_drop_column());
        }
        if self.eat_word("RENAME") {
            return Ok(self.parse_rename());
        }
        // MySQL: MODIFY [COLUMN] definition
        if self.eat_word("MODIFY") {
            self.eat_word("COLUMN");
            return Ok(self
                .parse_column()?
                .map(|column| Alteration::ReplaceColumn {
                    name: column.name.clone(),
                    column,
                }));
        }
        // MySQL: CHANGE [COLUMN] old_name definition
        if self.eat_word("CHANGE") {
            self.eat_word("COLUMN");
            let Some(name) = self.eat_ident() else {
                return Ok(None);
            };
            return Ok(self
                .parse_column()?
                .map(|column| Alteration::ReplaceColumn { name, column }));
        }
        // Owner, storage, triggers and the like change nothing drawn
        Ok(None)
    }

    /// Parse the rest of ADD: a constraint or [COLUMN] [IF NOT EXISTS] definition
    fn parse_add(&mut self) -> Result<Option<Alteration>, SqlParseError> {
        let named = self.current() == &Token::Constraint;
        self.skip_constraint_name();

        Ok(match self.current() {
            Token::Foreign => self
                .parse_foreign_key_constraint()?
                .map(Alteration::ForeignKey),
            Token::Primary => Some(Alteration::PrimaryKey(self.parse_primary_key()?)),
            Token::Ident(_) if !named => {
                self.eat_word("COLUMN");
                self.skip_if_not_exists();
                self.parse_column()?.map(Alteration::AddColumn)
            }
            // UNIQUE, CHECK, INDEX add nothing
            _ => None,
        })
    }

    /// Parse the rest of ALTER [COLUMN] name: its type, nullability or default
    fn parse_alter_column(&mut self) -> Result<Option<Alteration>, SqlParseError> {
        self.eat_word("COLUMN");
        let Some(column) = self.eat_ident() else {
            return Ok(None);
        };

        if self.eat_word("DROP") {
            return Ok(match self.current() {
                Token::Not => Some(Alteration::SetNotNull {
                    column,
                    not_null: false,
                }),
                Token::Default => Some(Alteration::SetDefault {
                    column,
                    default: None,
                }),
                _ => None,
            });
        }
        if self.eat_word("SET") {
            match self.current() {
                Token::Not => {
                    return Ok(Some(Alteration::SetNotNull {
                        column,
                        not_null: true,
                    }));
                }
                Token::Default => {
                    self.advance();
                    let default = Some(self.parse_default_value()?);
                    return Ok(Some(Alteration::SetDefault { column, default }));
                }
                _ => {}
            }
            // SET DATA TYPE
            self.eat_word("DATA");
        }
        if !self.eat_word("TYPE") {
            return Ok(None);
        }
        Ok(self
            .parse_type()
            .map(|typ| Alteration::SetType { column, typ }))
    }

    /// Parse the rest of DROP [COLUMN] [IF EXISTS] name
    fn parse_drop_column(&mut self) -> Option<Alteration> {
        self.eat_word("COLUMN");
        self.skip_if_not_exists();
        self.eat_ident().map(Alteration::DropColumn)
    }

    /// Parse the rest of RENAME TO name, or RENAME [COLUMN] name TO name
    fn parse_rename(&mut self) -> Option<Alteration> {
        if self.eat_word("TO") {
            return self.eat_ident().map(Alteration::RenameTable);
        }
        self.eat_word("COLUMN");
        let from = self.eat_ident()?;
        if !self.eat_word("TO") {
            return None;
        }
        let to = self.eat_ident()?;
        Some(Alteration::RenameColumn { from, to })
    }

    /// Skip IF [NOT] EXISTS
    fn skip_if_not_exists(&mut self) {
        if self.current() != &Token::If {
            return;
        }
        self.advance(); // IF
        if self.current() == &Token::Not {
            self.advance(); // NOT
        }
        if self.current() == &Token::Exists {
            self.advance(); // EXISTS
        }
    }
}

/// Whether a word after a column's type starts its options rather than continuing the type.
fn ends_type(word: &str) -> bool {
    ["GENERATED", "COLLATE"]
        .iter()
        .any(|w| word.eq_ignore_ascii_case(w))
}

/// What one item of a table body states.
enum TableItem {
    Column(Column),
    PrimaryKey(Vec<String>),
    ForeignKey(FkInfo),
}

/// What one action of ALTER TABLE does to its table.
enum Alteration {
    ForeignKey(FkInfo),
    PrimaryKey(Vec<String>),
    AddColumn(Column),
    DropColumn(String),
    ReplaceColumn {
        name: String,
        column: Column,
    },
    RenameColumn {
        from: String,
        to: String,
    },
    RenameTable(String),
    SetType {
        column: String,
        typ: String,
    },
    SetNotNull {
        column: String,
        not_null: bool,
    },
    SetDefault {
        column: String,
        default: Option<String>,
    },
}

/// Words that may stand between CREATE and TABLE.
const TABLE_KINDS: [&str; 5] = ["TEMPORARY", "TEMP", "UNLOGGED", "GLOBAL", "LOCAL"];

/// Mark `cols` as the primary key of `entity`.
fn add_primary_key(entity: &mut Entity, cols: Vec<String>) {
    for col in &mut entity.columns {
        if cols.contains(&col.name) && !col.modifiers.contains(&ColumnModifier::Pk) {
            col.modifiers.insert(0, ColumnModifier::Pk);
        }
    }
    if cols.len() > 1 {
        entity.constraints.push(Constraint::PrimaryKey(cols));
    }
}

/// Mark the columns of `fk` as foreign keys of `entity`.
fn add_foreign_key(entity: &mut Entity, fk: &FkInfo) {
    for col in &mut entity.columns {
        let is_fk = col
            .modifiers
            .iter()
            .any(|m| matches!(m, ColumnModifier::Fk { .. }));
        if fk.columns.contains(&col.name) && !is_fk {
            col.modifiers.push(ColumnModifier::Fk {
                target: fk.target.clone(),
                column: fk.target_column.clone(),
            });
        }
    }
}

fn column_mut<'a>(entities: &'a mut [Entity], table: &str, column: &str) -> Option<&'a mut Column> {
    entities
        .iter_mut()
        .find(|e| e.name == table)?
        .columns
        .iter_mut()
        .find(|c| c.name == column)
}

/// Apply one ALTER TABLE action to what the earlier statements declared.
fn alter(
    entities: &mut [Entity],
    fk_constraints: &mut Vec<(String, FkInfo)>,
    table: &str,
    alteration: Alteration,
) {
    match alteration {
        Alteration::ForeignKey(fk) => {
            if let Some(entity) = entities.iter_mut().find(|e| e.name == table) {
                add_foreign_key(entity, &fk);
            }
            fk_constraints.push((table.to_string(), fk));
        }
        Alteration::PrimaryKey(cols) => {
            if let Some(entity) = entities.iter_mut().find(|e| e.name == table) {
                add_primary_key(entity, cols);
            }
        }
        Alteration::AddColumn(column) => {
            if let Some(entity) = entities.iter_mut().find(|e| e.name == table) {
                entity.columns.push(column);
            }
        }
        Alteration::DropColumn(name) => {
            if let Some(entity) = entities.iter_mut().find(|e| e.name == table) {
                entity.columns.retain(|c| c.name != name);
            }
        }
        Alteration::ReplaceColumn { name, column } => {
            if let Some(col) = column_mut(entities, table, &name) {
                *col = column;
            }
        }
        Alteration::RenameColumn { from, to } => {
            for entity in entities.iter_mut() {
                let own = entity.name == table;
                for constraint in &mut entity.constraints {
                    if let Constraint::PrimaryKey(cols) = constraint
                        && own
                    {
                        for col in cols.iter_mut().filter(|c| **c == from) {
                            *col = to.clone();
                        }
                    }
                }
                for col in &mut entity.columns {
                    if own && col.name == from {
                        col.name = to.clone();
                    }
                    for modifier in &mut col.modifiers {
                        if let ColumnModifier::Fk { target, column } = modifier
                            && target == table
                            && *column == from
                        {
                            *column = to.clone();
                        }
                    }
                }
            }
        }
        Alteration::RenameTable(to) => {
            for (source, fk) in fk_constraints.iter_mut() {
                if source == table {
                    *source = to.clone();
                }
                if fk.target == table {
                    fk.target = to.clone();
                }
            }
            for entity in entities.iter_mut() {
                if entity.name == table {
                    entity.name = to.clone();
                }
                for modifier in entity.columns.iter_mut().flat_map(|c| &mut c.modifiers) {
                    if let ColumnModifier::Fk { target, .. } = modifier
                        && target == table
                    {
                        *target = to.clone();
                    }
                }
            }
        }
        Alteration::SetType { column, typ } => {
            if let Some(col) = column_mut(entities, table, &column) {
                col.typ = typ;
            }
        }
        Alteration::SetNotNull { column, not_null } => {
            if let Some(col) = column_mut(entities, table, &column) {
                col.modifiers.retain(|m| m != &ColumnModifier::NotNull);
                if not_null {
                    col.modifiers.push(ColumnModifier::NotNull);
                }
            }
        }
        Alteration::SetDefault { column, default } => {
            if let Some(col) = column_mut(entities, table, &column) {
                col.modifiers
                    .retain(|m| !matches!(m, ColumnModifier::Default(_)));
                if let Some(default) = default {
                    col.modifiers.push(ColumnModifier::Default(default));
                }
            }
        }
    }
}

struct FkInfo {
    columns: Vec<String>,
    target: String,
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
        assert!(
            user.columns[0]
                .modifiers
                .iter()
                .any(|m| matches!(m, ColumnModifier::Pk))
        );

        assert_eq!(user.columns[1].name, "email");
        assert!(
            user.columns[1]
                .modifiers
                .iter()
                .any(|m| matches!(m, ColumnModifier::NotNull))
        );
        assert!(
            user.columns[1]
                .modifiers
                .iter()
                .any(|m| matches!(m, ColumnModifier::Unique))
        );
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

    fn column_names(entity: &Entity) -> Vec<&str> {
        entity.columns.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn test_parse_identity_options_keep_following_columns() {
        let sql = r#"
            CREATE TABLE "t" (
                "id" integer PRIMARY KEY GENERATED ALWAYS AS IDENTITY (sequence name "t_id_seq" INCREMENT BY 1 MINVALUE 1 START WITH 1 CACHE 1),
                "n" integer GENERATED ALWAYS AS IDENTITY,
                "name" text NOT NULL,
                "created_at" timestamp with time zone DEFAULT now() NOT NULL
            );
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();
        let t = &schema.entities[0];

        assert_eq!(column_names(t), ["id", "n", "name", "created_at"]);
        assert_eq!(t.columns[1].typ, "int");
        assert_eq!(t.columns[3].typ, "timestamp");
    }

    #[test]
    fn test_parse_check_adds_no_column() {
        let sql = r#"
            CREATE TABLE "t" (
                "id" integer PRIMARY KEY,
                "age" integer CHECK ("age" > 0) NOT NULL,
                "status" text NOT NULL,
                CONSTRAINT "t_status_check" CHECK ("t"."status" in ('new', 'done'))
            );
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();
        let t = &schema.entities[0];

        assert_eq!(column_names(t), ["id", "age", "status"]);
        assert!(t.columns[1].modifiers.contains(&ColumnModifier::NotNull));
    }

    #[test]
    fn test_parse_enum_keeps_following_columns() {
        let sql = r#"
            CREATE TABLE t (
                status ENUM('new', 'done') NOT NULL,
                name VARCHAR(255)
            );
        "#;

        let schema = parse_sql(sql, Dialect::MySQL).unwrap();
        let t = &schema.entities[0];

        assert_eq!(column_names(t), ["status", "name"]);
        assert_eq!(t.columns[0].typ, "enum");
    }

    #[test]
    fn test_parse_alter_table_add_column() {
        let sql = r#"
            CREATE TABLE "t" ("id" integer PRIMARY KEY);
            ALTER TABLE "t" ADD COLUMN "meeting" text DEFAULT 'email' NOT NULL;
            ALTER TABLE "t" ADD CONSTRAINT "t_meeting_check" CHECK ("t"."meeting" in ('meet', 'email'));
            CREATE TABLE "u" ("id" integer PRIMARY KEY);
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();

        assert_eq!(schema.entities.len(), 2);
        assert_eq!(column_names(&schema.entities[0]), ["id", "meeting"]);
        assert_eq!(column_names(&schema.entities[1]), ["id"]);
    }

    #[test]
    fn test_unknown_syntax_stays_inside_its_item() {
        let sql = r#"
            CREATE TABLE a (
                id INT PRIMARY KEY,
                x INT FROB (1, 2) NOT NULL,
                FROB (x, y),
                EXCLUDE USING gist (x WITH &&),
                LIKE other INCLUDING ALL,
                name TEXT NOT NULL
            ) PARTITION BY RANGE (id);
            FROB ( , ) CREATE TABLE ( ;
            ALTER TABLE a FROB (1, 2), ADD COLUMN note TEXT, ADD COLUMN age INT;
            CREATE TABLE b (id INT PRIMARY KEY);
        "#;

        let schema = parse_sql(sql, Dialect::Generic).unwrap();

        assert_eq!(schema.entities.len(), 2);
        assert_eq!(
            column_names(&schema.entities[0]),
            ["id", "x", "name", "note", "age"]
        );
        assert_eq!(column_names(&schema.entities[1]), ["id"]);
    }

    #[test]
    fn test_parse_array_types() {
        let sql = r#"
            CREATE TEMPORARY TABLE t (
                tags text[] DEFAULT '{}' NOT NULL,
                grid integer[3][3],
                names varchar(20) ARRAY
            );
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();
        let t = &schema.entities[0];

        let types: Vec<&str> = t.columns.iter().map(|c| c.typ.as_str()).collect();
        assert_eq!(types, ["text[]", "int[][]", "varchar[]"]);
        assert!(t.columns[0].modifiers.contains(&ColumnModifier::NotNull));
    }

    #[test]
    fn test_parse_alter_table_actions() {
        let sql = r#"
            CREATE TABLE users (id integer NOT NULL, name text, old text);
            CREATE TABLE posts (id integer PRIMARY KEY, user_id integer REFERENCES users(id));
            CREATE TABLE gone (id integer REFERENCES users(id));
            ALTER TABLE ONLY users ADD CONSTRAINT users_pkey PRIMARY KEY (id);
            ALTER TABLE users
                DROP COLUMN old,
                RENAME COLUMN name TO full_name,
                ALTER COLUMN full_name SET NOT NULL,
                ALTER COLUMN full_name SET DEFAULT 'x',
                ALTER COLUMN id SET DATA TYPE bigint,
                OWNER TO someone;
            ALTER TABLE users RENAME COLUMN id TO uid;
            ALTER TABLE users RENAME TO accounts;
            DROP TABLE IF EXISTS gone;
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();
        assert_eq!(schema.entities.len(), 2);

        let accounts = &schema.entities[0];
        assert_eq!(accounts.name, "accounts");
        assert_eq!(column_names(accounts), ["uid", "full_name"]);
        assert_eq!(accounts.columns[0].typ, "bigint");
        assert_eq!(
            accounts.columns[0].modifiers,
            [ColumnModifier::Pk, ColumnModifier::NotNull]
        );
        assert_eq!(
            accounts.columns[1].modifiers,
            [
                ColumnModifier::NotNull,
                ColumnModifier::Default("x".to_string())
            ]
        );

        assert_eq!(schema.relationships.len(), 1);
        assert_eq!(schema.relationships[0].left, "accounts");
        assert_eq!(schema.relationships[0].right, "posts");
        assert!(
            schema.entities[1].columns[1]
                .modifiers
                .contains(&ColumnModifier::Fk {
                    target: "accounts".to_string(),
                    column: "uid".to_string(),
                })
        );
    }

    #[test]
    fn test_foreign_key_constraints_mark_their_columns() {
        let sql = r#"
            CREATE TABLE users (id integer PRIMARY KEY);
            CREATE TABLE posts (
                id integer PRIMARY KEY,
                author_id integer NOT NULL,
                FOREIGN KEY (author_id) REFERENCES users(id)
            );
            CREATE TABLE likes (id integer PRIMARY KEY, post_id integer);
            ALTER TABLE ONLY likes
                ADD CONSTRAINT likes_post_id_fk FOREIGN KEY (post_id) REFERENCES public.posts(id) ON DELETE CASCADE;
        "#;

        let schema = parse_sql(sql, Dialect::PostgreSQL).unwrap();

        let fk = |table: usize, column: usize| {
            schema.entities[table].columns[column]
                .modifiers
                .iter()
                .find(|m| matches!(m, ColumnModifier::Fk { .. }))
                .cloned()
        };
        assert_eq!(
            fk(1, 1),
            Some(ColumnModifier::Fk {
                target: "users".to_string(),
                column: "id".to_string(),
            })
        );
        assert_eq!(
            fk(2, 1),
            Some(ColumnModifier::Fk {
                target: "posts".to_string(),
                column: "id".to_string(),
            })
        );
        assert_eq!(schema.relationships.len(), 2);
    }

    #[test]
    fn test_unbalanced_parentheses_are_an_error() {
        for sql in [
            "CREATE TABLE t (id INT, name TEXT));",
            "CREATE TABLE t (id INT, name VARCHAR(10);",
            "CREATE TABLE t (id INT); ALTER TABLE t ADD COLUMN x DECIMAL(10, 2;",
        ] {
            assert!(
                matches!(
                    parse_sql(sql, Dialect::Generic),
                    Err(SqlParseError::UnbalancedParens(table)) if table == "t"
                ),
                "{sql}"
            );
        }

        assert!(matches!(
            parse_sql(
                "CREATE TABLE t (id INT, x INT ) ( NOT NULL);",
                Dialect::Generic
            ),
            Err(SqlParseError::UnexpectedToken(Token::LParen))
        ));
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
        assert!(
            user.columns[0]
                .modifiers
                .iter()
                .any(|m| matches!(m, ColumnModifier::Pk))
        );
    }
}
