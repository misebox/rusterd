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
        let tokens = Lexer::new(input).tokenize()?;
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
        let mut views = Vec::new();
        let mut arrangement = None;

        while *self.peek() != Token::Eof {
            if *self.peek() == Token::At {
                // Could be @hint.arrangement at top level
                if self.try_parse_arrangement()? {
                    arrangement = Some(self.parse_arrangement_block()?);
                } else {
                    return Err(ParseError::Unexpected(
                        self.peek().clone(),
                        "entity, rel, view, or @hint.arrangement",
                    ));
                }
            } else if self.check_ident("entity") {
                self.advance();
                entities.push(self.parse_entity()?);
            } else if self.check_ident("rel") {
                self.advance();
                relationships.extend(self.parse_rel_block()?);
            } else if self.check_ident("view") {
                self.advance();
                views.push(self.parse_view()?);
            } else {
                return Err(ParseError::Unexpected(
                    self.peek().clone(),
                    "entity, rel, view, or @hint.arrangement",
                ));
            }
        }

        Ok(Schema {
            entities,
            relationships,
            views,
            arrangement,
        })
    }

    /// Check if we're at @hint.arrangement and consume those tokens if so
    fn try_parse_arrangement(&mut self) -> Result<bool, ParseError> {
        if *self.peek() != Token::At {
            return Ok(false);
        }

        // Look ahead: @ hint . arrangement =
        let start_pos = self.pos;

        self.advance(); // @
        if !self.check_ident("hint") {
            self.pos = start_pos;
            return Ok(false);
        }
        self.advance(); // hint

        if *self.peek() != Token::Dot {
            self.pos = start_pos;
            return Ok(false);
        }
        self.advance(); // .

        if !self.check_ident("arrangement") {
            self.pos = start_pos;
            return Ok(false);
        }
        self.advance(); // arrangement

        if *self.peek() != Token::Eq {
            self.pos = start_pos;
            return Ok(false);
        }
        self.advance(); // =

        Ok(true)
    }

    /// Parse arrangement block: { Entity1 Entity2; Entity3 Entity4; ... }
    fn parse_arrangement_block(&mut self) -> Result<Vec<Vec<String>>, ParseError> {
        self.expect(Token::LBrace)?;

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut current_row: Vec<String> = Vec::new();

        while *self.peek() != Token::RBrace {
            match self.peek().clone() {
                Token::Ident(name) => {
                    self.advance();
                    current_row.push(name);
                }
                Token::Semicolon => {
                    self.advance();
                    if !current_row.is_empty() {
                        rows.push(current_row);
                        current_row = Vec::new();
                    }
                }
                tok => {
                    return Err(ParseError::Unexpected(tok, "entity name or semicolon"));
                }
            }
        }

        // Don't forget the last row (no trailing semicolon required)
        if !current_row.is_empty() {
            rows.push(current_row);
        }

        self.expect(Token::RBrace)?;
        Ok(rows)
    }

    fn parse_entity(&mut self) -> Result<Entity, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;

        let mut columns = Vec::new();
        let mut constraints = Vec::new();
        let mut hints = Vec::new();

        while *self.peek() != Token::RBrace {
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
                    // Parse arguments until )
                    loop {
                        match self.peek() {
                            Token::RParen => {
                                self.advance();
                                break;
                            }
                            Token::Eof => break,
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
            return Err(ParseError::Unexpected(
                self.peek().clone(),
                "references",
            ));
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
        self.expect(Token::LBrace)?;
        let mut rels = Vec::new();

        while *self.peek() != Token::RBrace {
            rels.push(self.parse_relationship()?);
        }

        self.expect(Token::RBrace)?;
        Ok(rels)
    }

    fn parse_relationship(&mut self) -> Result<Relationship, ParseError> {
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
            tok => Err(ParseError::Unexpected(tok, "cardinality (1, 0..1, *, 1..*)")),
        }
    }

    fn parse_view(&mut self) -> Result<View, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::LBrace)?;

        let mut includes = Vec::new();

        while *self.peek() != Token::RBrace {
            if self.check_ident("include") {
                self.advance();
                includes.extend(self.parse_ident_list()?);
            } else {
                return Err(ParseError::Unexpected(self.peek().clone(), "include"));
            }
        }

        self.expect(Token::RBrace)?;

        Ok(View { name, includes })
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
    fn test_parse_view() {
        let input = r#"
            view core {
                include User, Order, Product
            }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();
        assert_eq!(schema.views.len(), 1);
        assert_eq!(schema.views[0].includes, vec!["User", "Order", "Product"]);
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
    fn test_parse_arrangement() {
        let input = r#"
            @hint.arrangement = {
                Category Address Customer;
                Product Order Review Cart;
                ProductImage OrderItem CartItem Payment
            }

            entity Category { id int pk }
            entity Address { id int pk }
            entity Customer { id int pk }
            entity Product { id int pk }
            entity Order { id int pk }
            entity Review { id int pk }
            entity Cart { id int pk }
            entity ProductImage { id int pk }
            entity OrderItem { id int pk }
            entity CartItem { id int pk }
            entity Payment { id int pk }
        "#;
        let schema = Parser::new(input).unwrap().parse().unwrap();

        assert!(schema.arrangement.is_some());
        let arr = schema.arrangement.unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0], vec!["Category", "Address", "Customer"]);
        assert_eq!(arr[1], vec!["Product", "Order", "Review", "Cart"]);
        assert_eq!(arr[2], vec!["ProductImage", "OrderItem", "CartItem", "Payment"]);
    }
}
