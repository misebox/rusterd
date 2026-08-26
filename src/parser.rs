use crate::ast::*;
use crate::lexer::{LexError, Lexer, Token};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Lex error: {0}")]
    Lex(#[from] LexError),
    #[error("Unexpected token: {0:?}, expected {1}")]
    Unexpected(Token, &'static str),
    #[error("Unexpected end of input")]
    UnexpectedEof,
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(input: &str) -> Result<Self, ParseError> {
        let mut lexer = Lexer::new(input);
        lexer.set_preserve_newlines(true);
        let tokens = lexer.tokenize()?;
        Ok(Self { tokens, pos: 0 })
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> &Token {
        let tok = self.tokens.get(self.pos).unwrap_or(&Token::Eof);
        self.pos += 1;
        tok
    }

    /// Skip all newline tokens.
    fn skip_newlines(&mut self) {
        while self.peek() == &Token::Newline {
            self.pos += 1;
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.advance().clone() {
            Token::Ident(s) => Ok(s),
            tok => Err(ParseError::Unexpected(tok, "identifier")),
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let tok = self.advance().clone();
        if tok == expected {
            Ok(())
        } else {
            Err(ParseError::Unexpected(tok, "specific token"))
        }
    }

    fn check_ident(&self, name: &str) -> bool {
        matches!(self.peek(), Token::Ident(s) if s == name)
    }

    pub fn parse(&mut self) -> Result<Schema, ParseError> {
        let mut entities = Vec::new();
        let mut relationships = Vec::new();
        let mut focuses = Vec::new();
        let mut near: Vec<Vec<String>> = Vec::new();
        let mut omit: Vec<String> = Vec::new();
        let mut brief: Vec<String> = Vec::new();

        loop {
            self.skip_newlines();
            if *self.peek() == Token::Eof {
                break;
            }

            if *self.peek() == Token::At {
                match self.parse_hint_key()?.as_deref() {
                    Some("near") => near.push(self.parse_name_set()?),
                    Some("omit") => omit.extend(self.parse_name_set()?),
                    Some("brief") => brief.extend(self.parse_name_set()?),
                    _ => {
                        return Err(ParseError::Unexpected(
                            self.peek().clone(),
                            "entity, rel, focus, @hint.near, @hint.omit, or @hint.brief",
                        ));
                    }
                }
            } else if self.check_ident("entity") {
                self.advance();
                entities.push(self.parse_entity()?);
            } else if self.check_ident("rel") {
                self.advance();
                relationships.extend(self.parse_rel_block()?);
            } else if self.check_ident("focus") {
                self.advance();
                focuses.push(self.parse_focus()?);
            } else {
                return Err(ParseError::Unexpected(
                    self.peek().clone(),
                    "entity, rel, focus, @hint.near, @hint.omit, or @hint.brief",
                ));
            }
        }

        Ok(Schema {
            entities,
            relationships,
            focuses,
            near,
            omit,
            brief,
        })
    }

    /// Read `@hint.<key> = ` at the top level, and say which key it was.
    ///
    /// Nothing is consumed unless the whole opening is there, so a stray `@`
    /// still reports itself as unexpected rather than as a broken hint.
    fn parse_hint_key(&mut self) -> Result<Option<String>, ParseError> {
        let start = self.pos;
        let rewind = |parser: &mut Self| {
            parser.pos = start;
            Ok(None)
        };

        if *self.peek() != Token::At {
            return rewind(self);
        }
        self.advance();

        if !self.check_ident("hint") {
            return rewind(self);
        }
        self.advance();

        if *self.peek() != Token::Dot {
            return rewind(self);
        }
        self.advance();

        let Token::Ident(key) = self.peek().clone() else {
            return rewind(self);
        };
        self.advance();

        if *self.peek() != Token::Eq {
            return rewind(self);
        }
        self.advance();

        Ok(Some(key))
    }

    /// Parse `{ Entity, Entity, Entity }`. Commas, newlines and plain spaces
    /// all separate; which one is used says nothing.
    fn parse_name_set(&mut self) -> Result<Vec<String>, ParseError> {
        self.skip_newlines();
        self.expect(Token::LBrace)?;

        let mut names = Vec::new();
        loop {
            match self.peek().clone() {
                Token::RBrace => break,
                Token::Ident(name) => {
                    self.advance();
                    names.push(name);
                }
                Token::Comma | Token::Semicolon | Token::Newline => {
                    self.advance();
                }
                tok => return Err(ParseError::Unexpected(tok, "entity name")),
            }
        }
        self.expect(Token::RBrace)?;
        self.skip_newlines();

        Ok(names)
    }

    fn parse_entity(&mut self) -> Result<Entity, ParseError> {
        self.skip_newlines();
        let name = self.expect_ident()?;
        self.skip_newlines();
        self.expect(Token::LBrace)?;

        let mut columns = Vec::new();
        let mut constraints = Vec::new();
        let mut hints = Vec::new();

        loop {
            self.skip_newlines();
            if *self.peek() == Token::RBrace {
                break;
            }
            if *self.peek() == Token::At {
                hints.push(self.parse_hint()?);
            } else if self.check_ident("primary_key") {
                self.advance();
                constraints.push(self.parse_primary_key()?);
            } else if self.check_ident("foreign_key") {
                self.advance();
                constraints.push(self.parse_foreign_key()?);
            } else if self.check_ident("index") {
                self.advance();
                constraints.push(self.parse_index()?);
            } else {
                columns.push(self.parse_column()?);
            }
        }

        self.expect(Token::RBrace)?;

        Ok(Entity {
            name,
            columns,
            constraints,
            hints,
        })
    }

    fn parse_column(&mut self) -> Result<Column, ParseError> {
        let name = self.expect_ident()?;
        let typ = self.expect_ident()?;
        let mut modifiers = Vec::new();

        loop {
            // End of column definition on newline or closing brace
            if matches!(self.peek(), Token::Newline | Token::RBrace | Token::Eof) {
                break;
            }
            if self.check_ident("pk") {
                self.advance();
                modifiers.push(ColumnModifier::Pk);
            } else if self.check_ident("not") {
                self.advance();
                if self.check_ident("null") {
                    self.advance();
                    modifiers.push(ColumnModifier::NotNull);
                }
            } else if self.check_ident("unique") {
                self.advance();
                modifiers.push(ColumnModifier::Unique);
            } else if self.check_ident("default") {
                self.advance();
                let val = self.parse_default_value()?;
                modifiers.push(ColumnModifier::Default(val));
            } else if self.check_ident("fk") {
                self.advance();
                self.expect(Token::Arrow)?;
                let target = self.expect_ident()?;
                self.expect(Token::Dot)?;
                let column = self.expect_ident()?;
                modifiers.push(ColumnModifier::Fk { target, column });
            } else {
                break;
            }
        }

        Ok(Column {
            name,
            typ,
            modifiers,
        })
    }

    fn parse_default_value(&mut self) -> Result<String, ParseError> {
        match self.advance().clone() {
            Token::Ident(s) => {
                // Check for function call: IDENT()
                if *self.peek() == Token::LParen {
                    self.advance(); // consume (
                    let mut args = String::new();
                    // Parse arguments until the matching ), so that a nested
                    // call like coalesce(x, now()) keeps its own parentheses.
                    let mut depth = 1;
                    loop {
                        match self.peek() {
                            Token::Eof => break,
                            Token::RParen if depth == 1 => {
                                self.advance();
                                break;
                            }
                            _ => {
                                let tok = self.advance().clone();
                                match tok {
                                    Token::Ident(a) => args.push_str(&a),
                                    Token::Num(n) => args.push_str(&n.to_string()),
                                    Token::Str(st) => {
                                        args.push('"');
                                        args.push_str(&st);
                                        args.push('"');
                                    }
                                    Token::Comma => args.push_str(", "),
                                    Token::LParen => {
                                        depth += 1;
                                        args.push('(');
                                    }
                                    Token::RParen => {
                                        depth -= 1;
                                        args.push(')');
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Ok(format!("{}({})", s, args))
                } else {
                    Ok(s)
                }
            }
            Token::Str(s) => Ok(format!("\"{}\"", s)),
            Token::Num(n) => Ok(n.to_string()),
            tok => Err(ParseError::Unexpected(tok, "default value")),
        }
    }

    fn parse_hint(&mut self) -> Result<Hint, ParseError> {
        self.expect(Token::At)?;
        let mut key = self.expect_ident()?;

        while *self.peek() == Token::Dot {
            self.advance();
            key.push('.');
            key.push_str(&self.expect_ident()?);
        }

        self.expect(Token::Eq)?;

        let value = match self.advance().clone() {
            Token::Num(n) => HintValue::Int(n),
            Token::Str(s) => HintValue::Str(s),
            Token::Ident(s) => HintValue::Ident(s),
            tok => return Err(ParseError::Unexpected(tok, "hint value")),
        };

        Ok(Hint { key, value })
    }

    fn parse_primary_key(&mut self) -> Result<Constraint, ParseError> {
        self.expect(Token::LParen)?;
        let columns = self.parse_ident_list()?;
        self.expect(Token::RParen)?;
        Ok(Constraint::PrimaryKey(columns))
    }

    fn parse_foreign_key(&mut self) -> Result<Constraint, ParseError> {
        self.expect(Token::LParen)?;
        let columns = self.parse_ident_list()?;
        self.expect(Token::RParen)?;

        if !self.check_ident("references") {
            return Err(ParseError::Unexpected(self.peek().clone(), "references"));
        }
        self.advance();

        let target = self.expect_ident()?;
        self.expect(Token::LParen)?;
        let target_columns = self.parse_ident_list()?;
        self.expect(Token::RParen)?;

        let mut on_delete = None;
        let mut on_update = None;

        while self.check_ident("on") {
            self.advance();
            if self.check_ident("delete") {
                self.advance();
                on_delete = Some(self.expect_ident()?);
            } else if self.check_ident("update") {
                self.advance();
                on_update = Some(self.expect_ident()?);
            }
        }

        Ok(Constraint::ForeignKey {
            columns,
            target,
            target_columns,
            on_delete,
            on_update,
        })
    }

    fn parse_index(&mut self) -> Result<Constraint, ParseError> {
        self.expect(Token::LParen)?;
        let columns = self.parse_ident_list()?;
        self.expect(Token::RParen)?;

        let mut name = None;
        if *self.peek() == Token::LBracket {
            self.advance();
            if self.check_ident("name") {
                self.advance();
                self.expect(Token::Eq)?;
                name = Some(self.expect_ident()?);
            }
            self.expect(Token::RBracket)?;
        }

        Ok(Constraint::Index { columns, name })
    }

    fn parse_ident_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut list = vec![self.expect_ident()?];
        while *self.peek() == Token::Comma {
            self.advance();
            list.push(self.expect_ident()?);
        }
        Ok(list)
    }

    fn parse_rel_block(&mut self) -> Result<Vec<Relationship>, ParseError> {
        self.skip_newlines();
        self.expect(Token::LBrace)?;
        let mut rels = Vec::new();

        loop {
            self.skip_newlines();
            if *self.peek() == Token::RBrace {
                break;
            }
            rels.push(self.parse_relationship()?);
        }

        self.expect(Token::RBrace)?;
        Ok(rels)
    }

    fn parse_relationship(&mut self) -> Result<Relationship, ParseError> {
        self.skip_newlines();
        let left = self.expect_ident()?;
        let left_cardinality = self.parse_cardinality()?;
        self.expect(Token::Dash)?;
        let right_cardinality = self.parse_cardinality()?;
        let right = self.expect_ident()?;

        let mut label = None;
        let mut role = None;

        if *self.peek() == Token::Colon {
            self.advance();
            match self.advance().clone() {
                Token::Str(s) => label = Some(s),
                tok => return Err(ParseError::Unexpected(tok, "string label")),
            }
        }

        if self.check_ident("as") {
            self.advance();
            role = Some(self.expect_ident()?);
        }

        Ok(Relationship {
            left,
            left_cardinality,
            right,
            right_cardinality,
            label,
            role,
        })
    }

    fn parse_cardinality(&mut self) -> Result<Cardinality, ParseError> {
        match self.peek().clone() {
            Token::Star => {
                self.advance();
                Ok(Cardinality::Many)
            }
            Token::Num(0) => {
                self.advance();
                self.expect(Token::DotDot)?;
                match self.advance().clone() {
                    Token::Num(1) => Ok(Cardinality::ZeroOrOne),
                    tok => Err(ParseError::Unexpected(tok, "1 after 0..")),
                }
            }
            Token::Num(1) => {
                self.advance();
                if *self.peek() == Token::DotDot {
                    self.advance();
                    self.expect(Token::Star)?;
                    Ok(Cardinality::OneOrMore)
                } else {
                    Ok(Cardinality::One)
                }
            }
            tok => Err(ParseError::Unexpected(
                tok,
                "cardinality (1, 0..1, *, 1..*)",
            )),
        }
    }

    fn parse_focus(&mut self) -> Result<Focus, ParseError> {
        self.skip_newlines();
        let name = self.expect_ident()?;
        self.skip_newlines();
        self.expect(Token::LBrace)?;

        let mut includes = Vec::new();

        loop {
            self.skip_newlines();
            if *self.peek() == Token::RBrace {
                break;
            }
            if self.check_ident("include") {
                self.advance();
                includes.extend(self.parse_ident_list()?);
            } else {
                return Err(ParseError::Unexpected(self.peek().clone(), "include"));
            }
        }

        self.expect(Token::RBrace)?;

        Ok(Focus { name, includes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entity() {
        let input = r#"
            entity User {
                id int pk
                name string not null
                email string unique
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        assert_eq!(schema.entities.len(), 1);
        assert_eq!(schema.entities[0].name, "User");
        assert_eq!(schema.entities[0].columns.len(), 3);
    }

    #[test]
    fn test_parse_relationship() {
        let input = r#"
            rel {
                User 1 -- * Order : "places"
                User 0..1 -- 1..* Post as author
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        assert_eq!(schema.relationships.len(), 2);
        assert_eq!(schema.relationships[0].left, "User");
        assert_eq!(schema.relationships[0].label, Some("places".into()));
        assert_eq!(schema.relationships[1].role, Some("author".into()));
    }

    #[test]
    fn test_parse_focus() {
        let input = r#"
            focus core {
                include User, Order, Product
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        assert_eq!(schema.focuses.len(), 1);
        assert_eq!(schema.focuses[0].includes, vec!["User", "Order", "Product"]);
    }

    #[test]
    fn test_parse_unicode() {
        let input = r#"
            entity ユーザー {
                名前 文字列 not null
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        assert_eq!(schema.entities[0].name, "ユーザー");
        assert_eq!(schema.entities[0].columns[0].name, "名前");
    }

    #[test]
    fn reads_the_sets_a_diagram_asks_for() {
        let input = r#"
            @hint.near = { Order, OrderItem, Payment }
            @hint.near = {
                User
                UserProfile
            }
            @hint.omit = { migrations }
            @hint.brief = { audit_logs, events }

            entity Order { id int pk }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();

        assert_eq!(
            schema.near,
            vec![
                vec!["Order", "OrderItem", "Payment"],
                vec!["User", "UserProfile"],
            ]
        );
        assert_eq!(schema.omit, vec!["migrations"]);
        assert_eq!(schema.brief, vec!["audit_logs", "events"]);
    }

    #[test]
    fn refuses_a_hint_it_does_not_know() {
        let input = "@hint.arrangement = { A B }\nentity A { id int pk }";
        assert!(Parser::new(input).unwrap().parse().is_err());
    }
}
