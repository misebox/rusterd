//! SQL lexer for tokenizing CREATE TABLE statements.

use std::iter::Peekable;
use std::str::Chars;

/// SQL token types.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Create,
    Alter,
    Add,
    Table,
    Only,
    Primary,
    Key,
    Foreign,
    References,
    Not,
    Null,
    Unique,
    Default,
    On,
    Delete,
    Update,
    Cascade,
    SetNull,
    SetDefault,
    Restrict,
    NoAction,
    Constraint,
    Index,
    If,
    Exists,
    Auto,       // For AUTO_INCREMENT
    Increment,
    Serial,     // PostgreSQL
    Check,

    // Identifiers and literals
    Ident(String),
    Str(String),
    Num(String),

    // Symbols
    LParen,
    RParen,
    Comma,
    Semicolon,
    Dot,

    // End of input
    Eof,
}

/// SQL lexer.
pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    current_char: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut chars = input.chars().peekable();
        let current_char = chars.next();
        Self { chars, current_char }
    }

    fn advance(&mut self) {
        self.current_char = self.chars.next();
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.current_char {
            if c == '\n' {
                self.advance();
                break;
            }
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        self.advance(); // skip *
        while let Some(c) = self.current_char {
            if c == '*' {
                self.advance();
                if self.current_char == Some('/') {
                    self.advance();
                    break;
                }
            } else {
                self.advance();
            }
        }
    }

    fn read_identifier(&mut self) -> String {
        let mut ident = String::new();
        while let Some(c) = self.current_char {
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }
        ident
    }

    fn read_quoted_identifier(&mut self, quote: char) -> String {
        self.advance(); // skip opening quote
        let mut ident = String::new();
        while let Some(c) = self.current_char {
            if c == quote {
                // Check for escaped quote (doubled)
                if self.peek() == Some(&quote) {
                    ident.push(c);
                    self.advance();
                    self.advance();
                } else {
                    self.advance(); // skip closing quote
                    break;
                }
            } else {
                ident.push(c);
                self.advance();
            }
        }
        ident
    }

    fn read_string(&mut self) -> String {
        let quote = self.current_char.unwrap();
        self.advance(); // skip opening quote
        let mut s = String::new();
        while let Some(c) = self.current_char {
            if c == quote {
                // Check for escaped quote
                if self.peek() == Some(&quote) {
                    s.push(c);
                    self.advance();
                    self.advance();
                } else {
                    self.advance(); // skip closing quote
                    break;
                }
            } else if c == '\\' {
                self.advance();
                if let Some(escaped) = self.current_char {
                    match escaped {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        _ => s.push(escaped),
                    }
                    self.advance();
                }
            } else {
                s.push(c);
                self.advance();
            }
        }
        s
    }

    fn read_number(&mut self) -> String {
        let mut num = String::new();
        let mut has_dot = false;

        // Handle negative sign
        if self.current_char == Some('-') {
            num.push('-');
            self.advance();
        }

        while let Some(c) = self.current_char {
            if c.is_ascii_digit() {
                num.push(c);
                self.advance();
            } else if c == '.' && !has_dot {
                has_dot = true;
                num.push(c);
                self.advance();
            } else {
                break;
            }
        }
        num
    }

    fn keyword_or_ident(&self, s: &str) -> Token {
        match s.to_uppercase().as_str() {
            "CREATE" => Token::Create,
            "ALTER" => Token::Alter,
            "ADD" => Token::Add,
            "TABLE" => Token::Table,
            "ONLY" => Token::Only,
            "PRIMARY" => Token::Primary,
            "KEY" => Token::Key,
            "FOREIGN" => Token::Foreign,
            "REFERENCES" => Token::References,
            "NOT" => Token::Not,
            "NULL" => Token::Null,
            "UNIQUE" => Token::Unique,
            "DEFAULT" => Token::Default,
            "ON" => Token::On,
            "DELETE" => Token::Delete,
            "UPDATE" => Token::Update,
            "CASCADE" => Token::Cascade,
            "RESTRICT" => Token::Restrict,
            "CONSTRAINT" => Token::Constraint,
            "INDEX" => Token::Index,
            "IF" => Token::If,
            "EXISTS" => Token::Exists,
            "AUTO_INCREMENT" => Token::Increment,
            "AUTO" => Token::Auto,
            "INCREMENT" => Token::Increment,
            "SERIAL" => Token::Serial,
            "BIGSERIAL" => Token::Serial,
            "SMALLSERIAL" => Token::Serial,
            "CHECK" => Token::Check,
            "SET" => {
                // Could be SET NULL or SET DEFAULT
                Token::Ident(s.to_string())
            }
            "NO" => Token::Ident(s.to_string()),
            "ACTION" => Token::Ident(s.to_string()),
            _ => Token::Ident(s.to_string()),
        }
    }

    pub fn next_token(&mut self) -> Token {
        loop {
            self.skip_whitespace();

            match self.current_char {
                None => return Token::Eof,

                Some('-') => {
                    if self.peek() == Some(&'-') {
                        self.skip_line_comment();
                        continue;
                    } else if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        return Token::Num(self.read_number());
                    } else {
                        self.advance();
                        continue; // Skip standalone dash
                    }
                }

                Some('/') => {
                    if self.peek() == Some(&'*') {
                        self.advance();
                        self.skip_block_comment();
                        continue;
                    } else {
                        self.advance();
                        continue;
                    }
                }

                Some('#') => {
                    self.skip_line_comment();
                    continue;
                }

                Some('(') => {
                    self.advance();
                    return Token::LParen;
                }
                Some(')') => {
                    self.advance();
                    return Token::RParen;
                }
                Some(',') => {
                    self.advance();
                    return Token::Comma;
                }
                Some(';') => {
                    self.advance();
                    return Token::Semicolon;
                }
                Some('.') => {
                    self.advance();
                    return Token::Dot;
                }

                Some('"') => {
                    let ident = self.read_quoted_identifier('"');
                    return Token::Ident(ident);
                }
                Some('`') => {
                    let ident = self.read_quoted_identifier('`');
                    return Token::Ident(ident);
                }
                Some('[') => {
                    // SQL Server style [identifier]
                    self.advance();
                    let mut ident = String::new();
                    while let Some(c) = self.current_char {
                        if c == ']' {
                            self.advance();
                            break;
                        }
                        ident.push(c);
                        self.advance();
                    }
                    return Token::Ident(ident);
                }

                Some('\'') => {
                    let s = self.read_string();
                    return Token::Str(s);
                }

                Some(c) if c.is_ascii_digit() => {
                    return Token::Num(self.read_number());
                }

                Some(c) if c.is_alphabetic() || c == '_' => {
                    let ident = self.read_identifier();
                    return self.keyword_or_ident(&ident);
                }

                Some(_) => {
                    // Skip unknown characters
                    self.advance();
                    continue;
                }
            }
        }
    }

    /// Collect all tokens.
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            if token == Token::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_create_table() {
        let sql = "CREATE TABLE users (id INT);";
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0], Token::Create);
        assert_eq!(tokens[1], Token::Table);
        assert_eq!(tokens[2], Token::Ident("users".to_string()));
        assert_eq!(tokens[3], Token::LParen);
        assert_eq!(tokens[4], Token::Ident("id".to_string()));
        assert_eq!(tokens[5], Token::Ident("INT".to_string()));
        assert_eq!(tokens[6], Token::RParen);
        assert_eq!(tokens[7], Token::Semicolon);
    }

    #[test]
    fn test_quoted_identifiers() {
        let sql = r#"CREATE TABLE "User Table" (`column name` INT);"#;
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[2], Token::Ident("User Table".to_string()));
        assert_eq!(tokens[4], Token::Ident("column name".to_string()));
    }

    #[test]
    fn test_comments() {
        let sql = "-- comment\nCREATE /* block */ TABLE t (id INT);";
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize();

        assert_eq!(tokens[0], Token::Create);
        assert_eq!(tokens[1], Token::Table);
    }
}
