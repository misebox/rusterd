use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

/// Where something was written, counted the way an editor counts: the first
/// line is 1, and a column is a character rather than a byte, so a name in
/// Japanese lands where the cursor does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub line: usize,
    pub column: usize,
}

impl Default for Place {
    fn default() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Str(String),
    Num(i64),

    LBrace,    // {
    RBrace,    // }
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Semicolon, // ;
    Colon,     // :
    Eq,        // =
    At,        // @
    Star,      // *
    Dot,       // .
    Arrow,     // ->
    Dash,      // --
    DotDot,    // ..
    Newline,   // \n (preserved in certain contexts)

    Eof,
}

impl fmt::Display for Token {
    /// How a token is named in an error: as the reader wrote it, where that
    /// is a thing they wrote, and by description where it is not.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Ident(name) => write!(f, "`{name}`"),
            Token::Str(text) => write!(f, "\"{text}\""),
            Token::Num(n) => write!(f, "{n}"),
            Token::LBrace => write!(f, "`{{`"),
            Token::RBrace => write!(f, "`}}`"),
            Token::LParen => write!(f, "`(`"),
            Token::RParen => write!(f, "`)`"),
            Token::LBracket => write!(f, "`[`"),
            Token::RBracket => write!(f, "`]`"),
            Token::Comma => write!(f, "`,`"),
            Token::Semicolon => write!(f, "`;`"),
            Token::Colon => write!(f, "`:`"),
            Token::Eq => write!(f, "`=`"),
            Token::At => write!(f, "`@`"),
            Token::Star => write!(f, "`*`"),
            Token::Dot => write!(f, "`.`"),
            Token::Arrow => write!(f, "`->`"),
            Token::Dash => write!(f, "`--`"),
            Token::DotDot => write!(f, "`..`"),
            Token::Newline => write!(f, "the end of the line"),
            Token::Eof => write!(f, "the end of the file"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LexError {
    #[error("{1}: this file cannot hold the character {0:?}")]
    UnexpectedChar(char, Place),
    #[error("{0}: this string is never closed")]
    UnterminatedString(Place),
    #[error("{1}: {0} is not a number this reads")]
    InvalidNumber(String, Place),
}

/// The tokens of a document, and where each of them was written. The two are
/// read together: an error names a token, and a reader needs the line.
pub struct Tokens {
    pub tokens: Vec<Token>,
    pub places: Vec<Place>,
}

pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    preserve_newlines: bool,
    place: Place,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            preserve_newlines: false,
            place: Place::default(),
        }
    }

    /// The next character, counted. Every character this lexer reads goes
    /// through here, which is what keeps `place` honest.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.place = Place {
                line: self.place.line + 1,
                column: 1,
            };
        } else {
            self.place.column += 1;
        }
        Some(c)
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
                    self.bump();
                }
                Some('#') => {
                    while let Some(&c) = self.chars.peek() {
                        self.bump();
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
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    fn read_string(&mut self) -> Result<String, LexError> {
        let mut s = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(s),
                Some('\\') => {
                    if let Some(c) = self.bump() {
                        match c {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            _ => s.push(c),
                        }
                    }
                }
                Some(c) => s.push(c),
                None => return Err(LexError::UnterminatedString(self.place)),
            }
        }
    }

    fn read_number(&mut self, first: char) -> Result<i64, LexError> {
        let mut s = String::from(first);
        while let Some(&c) = self.chars.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let place = self.place;
        s.parse().map_err(|_| LexError::InvalidNumber(s, place))
    }

    /// The next token, and where it starts.
    pub fn next_token(&mut self) -> Result<(Token, Place), LexError> {
        self.skip_whitespace_and_comments();

        let place = self.place;
        let c = match self.bump() {
            Some(c) => c,
            None => return Ok((Token::Eof, place)),
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
                    self.bump();
                    Token::DotDot
                } else {
                    Token::Dot
                }
            }
            '-' => {
                if self.chars.peek() == Some(&'-') {
                    self.bump();
                    Token::Dash
                } else if self.chars.peek() == Some(&'>') {
                    self.bump();
                    Token::Arrow
                } else {
                    return Err(LexError::UnexpectedChar(c, place));
                }
            }
            '"' => Token::Str(self.read_string()?),
            c if c.is_ascii_digit() => Token::Num(self.read_number(c)?),
            c if c.is_alphabetic() || c == '_' => Token::Ident(self.read_ident(c)),
            _ => return Err(LexError::UnexpectedChar(c, place)),
        };

        Ok((tok, place))
    }

    pub fn tokenize(mut self) -> Result<Tokens, LexError> {
        let mut tokens = Vec::new();
        let mut places = Vec::new();
        loop {
            let (token, place) = self.next_token()?;
            let end = token == Token::Eof;
            tokens.push(token);
            places.push(place);
            if end {
                break;
            }
        }
        Ok(Tokens { tokens, places })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let tokens = Lexer::new("entity User { }").tokenize().unwrap().tokens;
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
        let read = Lexer::new("entity ユーザー { 名前 string }")
            .tokenize()
            .unwrap();
        assert_eq!(read.tokens[1], Token::Ident("ユーザー".into()));
        assert_eq!(read.tokens[3], Token::Ident("名前".into()));
        // Columns are characters, so a name in Japanese is where it looks.
        assert_eq!(
            read.places[3],
            Place {
                line: 1,
                column: 15
            }
        );
    }

    #[test]
    fn test_comments() {
        let input = "# comment\nentity User { # inline\n}";
        let tokens = Lexer::new(input).tokenize().unwrap().tokens;
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
        let tokens = Lexer::new("1 0..1 * 1..*").tokenize().unwrap().tokens;
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
        let tokens = Lexer::new("-- -> : = @ ;").tokenize().unwrap().tokens;
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
