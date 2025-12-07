use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Str(String),
    Num(i64),

    LBrace,   // {
    RBrace,   // }
    LParen,   // (
    RParen,   // )
    LBracket, // [
    RBracket, // ]
    Comma,     // ,
    Semicolon, // ;
    Colon,     // :
    Eq,        // =
    At,       // @
    Star,     // *
    Dot,      // .
    Arrow,    // ->
    Dash,     // --
    DotDot,   // ..
    Newline,  // \n (preserved in certain contexts)

    Eof,
}

#[derive(Debug, thiserror::Error)]
pub enum LexError {
    #[error("Unexpected character: {0}")]
    UnexpectedChar(char),
    #[error("Unterminated string")]
    UnterminatedString,
    #[error("Invalid number: {0}")]
    InvalidNumber(String),
}

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    preserve_newlines: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            preserve_newlines: false,
        }
    }

    /// Enable newline preservation (for arrangement blocks).
    pub fn set_preserve_newlines(&mut self, preserve: bool) {
        self.preserve_newlines = preserve;
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.chars.peek() {
                Some('\n') if self.preserve_newlines => {
                    // Don't skip newlines when preserving
                    break;
                }
                Some(c) if c.is_whitespace() => {
                    self.chars.next();
                }
                Some('#') => {
                    while let Some(&c) = self.chars.peek() {
                        self.chars.next();
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn read_ident(&mut self, first: char) -> String {
        let mut s = String::from(first);
        while let Some(&c) = self.chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        s
    }

    fn read_string(&mut self) -> Result<String, LexError> {
        let mut s = String::new();
        loop {
            match self.chars.next() {
                Some('"') => return Ok(s),
                Some('\\') => {
                    if let Some(c) = self.chars.next() {
                        match c {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            _ => s.push(c),
                        }
                    }
                }
                Some(c) => s.push(c),
                None => return Err(LexError::UnterminatedString),
            }
        }
    }

    fn read_number(&mut self, first: char) -> Result<i64, LexError> {
        let mut s = String::from(first);
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.chars.next();
            } else {
                break;
            }
        }
        s.parse().map_err(|_| LexError::InvalidNumber(s))
    }

    pub fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace_and_comments();

        let c = match self.chars.next() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };

        let tok = match c {
            '\n' if self.preserve_newlines => Token::Newline,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            ':' => Token::Colon,
            '=' => Token::Eq,
            '@' => Token::At,
            '*' => Token::Star,
            '.' => {
                if self.chars.peek() == Some(&'.') {
                    self.chars.next();
                    Token::DotDot
                } else {
                    Token::Dot
                }
            }
            '-' => {
                if self.chars.peek() == Some(&'-') {
                    self.chars.next();
                    Token::Dash
                } else if self.chars.peek() == Some(&'>') {
                    self.chars.next();
                    Token::Arrow
                } else {
                    return Err(LexError::UnexpectedChar(c));
                }
            }
            '"' => Token::Str(self.read_string()?),
            c if c.is_ascii_digit() => Token::Num(self.read_number(c)?),
            c if c.is_alphabetic() || c == '_' => Token::Ident(self.read_ident(c)),
            _ => return Err(LexError::UnexpectedChar(c)),
        };

        Ok(tok)
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            if tok == Token::Eof {
                tokens.push(tok);
                break;
            }
            tokens.push(tok);
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let tokens = Lexer::new("entity User { }").tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Ident("entity".into()),
                Token::Ident("User".into()),
                Token::LBrace,
                Token::RBrace,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_unicode_ident() {
        let tokens = Lexer::new("entity ユーザー { 名前 string }").tokenize().unwrap();
        assert_eq!(tokens[1], Token::Ident("ユーザー".into()));
        assert_eq!(tokens[3], Token::Ident("名前".into()));
    }

    #[test]
    fn test_comments() {
        let input = "# comment\nentity User { # inline\n}";
        let tokens = Lexer::new(input).tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Ident("entity".into()),
                Token::Ident("User".into()),
                Token::LBrace,
                Token::RBrace,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_cardinality_tokens() {
        let tokens = Lexer::new("1 0..1 * 1..*").tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Num(1),
                Token::Num(0),
                Token::DotDot,
                Token::Num(1),
                Token::Star,
                Token::Num(1),
                Token::DotDot,
                Token::Star,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn test_symbols() {
        let tokens = Lexer::new("-- -> : = @ ;").tokenize().unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Dash,
                Token::Arrow,
                Token::Colon,
                Token::Eq,
                Token::At,
                Token::Semicolon,
                Token::Eof,
            ]
        );
    }
}
