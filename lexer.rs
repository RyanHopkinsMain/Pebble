//! Source text -> tokens.

use crate::error::{Error, Pos};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Number(f64),
    Str(String),
    Ident(String),
    // keywords
    Let,
    Fn,
    Return,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    True,
    False,
    Nil,
    And,
    Or,
    Not,
    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, Error> {
    let chars: Vec<char> = src.chars().collect();
    let mut tokens = Vec::new();
    let (mut i, mut line, mut col) = (0, 1, 1);

    while i < chars.len() {
        let c = chars[i];
        let pos = Pos { line, col };
        let advance = |i: &mut usize, col: &mut usize, n: usize| {
            *i += n;
            *col += n;
        };
        match c {
            '\n' => {
                i += 1;
                line += 1;
                col = 1;
            }
            c if c.is_whitespace() => advance(&mut i, &mut col, 1),
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '0'..='9' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '_') {
                    i += 1;
                }
                if chars.get(i) == Some(&'.')
                    && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit())
                {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().filter(|&&c| c != '_').collect();
                col += i - start;
                let n = text
                    .parse()
                    .map_err(|_| Error::syntax(format!("bad number '{text}'"), pos))?;
                tokens.push(Token {
                    tok: Tok::Number(n),
                    pos,
                });
            }
            '"' => {
                let mut s = String::new();
                i += 1;
                col += 1;
                loop {
                    match chars.get(i) {
                        None | Some('\n') => return Err(Error::syntax("unterminated string", pos)),
                        Some('"') => break,
                        Some('\\') => {
                            let escaped = match chars.get(i + 1) {
                                Some('n') => '\n',
                                Some('t') => '\t',
                                Some('"') => '"',
                                Some('\\') => '\\',
                                Some(other) => {
                                    let at = Pos { line, col };
                                    return Err(Error::syntax(
                                        format!("unknown escape '\\{other}'"),
                                        at,
                                    ));
                                }
                                None => return Err(Error::syntax("unterminated string", pos)),
                            };
                            s.push(escaped);
                            i += 2;
                            col += 2;
                        }
                        Some(&c) => {
                            s.push(c);
                            i += 1;
                            col += 1;
                        }
                    }
                }
                i += 1;
                col += 1;
                tokens.push(Token {
                    tok: Tok::Str(s),
                    pos,
                });
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                col += i - start;
                let word: String = chars[start..i].iter().collect();
                let tok = match word.as_str() {
                    "let" => Tok::Let,
                    "fn" => Tok::Fn,
                    "return" => Tok::Return,
                    "if" => Tok::If,
                    "else" => Tok::Else,
                    "while" => Tok::While,
                    "for" => Tok::For,
                    "in" => Tok::In,
                    "break" => Tok::Break,
                    "continue" => Tok::Continue,
                    "true" => Tok::True,
                    "false" => Tok::False,
                    "nil" => Tok::Nil,
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    _ => Tok::Ident(word),
                };
                tokens.push(Token { tok, pos });
            }
            _ => {
                let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
                let (tok, len) = match two.as_str() {
                    "==" => (Tok::Eq, 2),
                    "!=" => (Tok::NotEq, 2),
                    "<=" => (Tok::Le, 2),
                    ">=" => (Tok::Ge, 2),
                    _ => (
                        match c {
                            '(' => Tok::LParen,
                            ')' => Tok::RParen,
                            '{' => Tok::LBrace,
                            '}' => Tok::RBrace,
                            '[' => Tok::LBracket,
                            ']' => Tok::RBracket,
                            ',' => Tok::Comma,
                            ':' => Tok::Colon,
                            ';' => Tok::Semicolon,
                            '+' => Tok::Plus,
                            '-' => Tok::Minus,
                            '*' => Tok::Star,
                            '/' => Tok::Slash,
                            '%' => Tok::Percent,
                            '=' => Tok::Assign,
                            '<' => Tok::Lt,
                            '>' => Tok::Gt,
                            _ => {
                                return Err(Error::syntax(
                                    format!("unexpected character '{c}'"),
                                    pos,
                                ))
                            }
                        },
                        1,
                    ),
                };
                advance(&mut i, &mut col, len);
                tokens.push(Token { tok, pos });
            }
        }
    }
    tokens.push(Token {
        tok: Tok::Eof,
        pos: Pos { line, col },
    });
    Ok(tokens)
}
