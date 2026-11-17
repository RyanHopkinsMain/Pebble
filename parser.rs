//! Tokens -> syntax tree. Statements by recursive descent, expressions by
//! Pratt parsing ("top-down operator precedence"): each operator has a binding
//! power, and `expression(min_bp)` keeps consuming operators that bind tighter
//! than `min_bp`. That one loop handles precedence and associativity.

use std::rc::Rc;

use crate::ast::{BinOp, Expr, FunctionDef, Stmt, UnOp};
use crate::error::{Error, Pos};
use crate::lexer::{Tok, Token};

pub fn parse(tokens: Vec<Token>) -> Result<Vec<Stmt>, Error> {
    let mut p = Parser {
        tokens,
        i: 0,
        loops: 0,
        functions: 0,
    };
    let mut program = Vec::new();
    while p.peek() != &Tok::Eof {
        program.push(p.statement()?);
    }
    Ok(program)
}

struct Parser {
    tokens: Vec<Token>,
    i: usize,
    loops: usize,     // to reject `break` outside a loop
    functions: usize, // to reject `return` outside a function
}

// Binding powers, weakest to strongest.
const OR: u8 = 1;
const AND: u8 = 2;
const EQUALITY: u8 = 3;
const COMPARISON: u8 = 4;
const TERM: u8 = 5;
const FACTOR: u8 = 6;
const UNARY: u8 = 7;
const POSTFIX: u8 = 8; // calls and indexing

impl Parser {
    fn peek(&self) -> &Tok {
        &self.tokens[self.i].tok
    }

    fn pos(&self) -> Pos {
        self.tokens[self.i].pos
    }

    fn next(&mut self) -> Token {
        let t = self.tokens[self.i].clone();
        if t.tok != Tok::Eof {
            self.i += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == tok {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: Tok, what: &str) -> Result<Pos, Error> {
        let pos = self.pos();
        if self.eat(&tok) {
            Ok(pos)
        } else {
            Err(Error::syntax(
                format!("expected {what}, found {}", describe(self.peek())),
                pos,
            ))
        }
    }

    fn ident(&mut self, what: &str) -> Result<String, Error> {
        match self.peek().clone() {
            Tok::Ident(name) => {
                self.i += 1;
                Ok(name)
            }
            other => Err(Error::syntax(
                format!("expected {what}, found {}", describe(&other)),
                self.pos(),
            )),
        }
    }

    // ------------------------------------------------------------- statements

    fn statement(&mut self) -> Result<Stmt, Error> {
        let pos = self.pos();
        match self.peek() {
            Tok::Let => {
                self.i += 1;
                let name = self.ident("a variable name")?;
                self.expect(Tok::Assign, "'=' after the variable name")?;
                let value = self.expression(0)?;
                self.semicolon()?;
                Ok(Stmt::Let(name, value))
            }
            Tok::Fn
                if matches!(
                    self.tokens.get(self.i + 1).map(|t| &t.tok),
                    Some(Tok::Ident(_))
                ) =>
            {
                self.i += 1;
                let name = self.ident("a function name")?;
                Ok(Stmt::Fn(self.function(name, pos)?))
            }
            Tok::If => self.if_statement(),
            Tok::While => {
                self.i += 1;
                let cond = self.expression(0)?;
                let body = self.loop_body()?;
                Ok(Stmt::While(cond, body))
            }
            Tok::For => {
                self.i += 1;
                let var = self.ident("a loop variable")?;
                self.expect(Tok::In, "'in'")?;
                let iterable = self.expression(0)?;
                let body = self.loop_body()?;
                Ok(Stmt::For(var, iterable, body, pos))
            }
            Tok::Return => {
                self.i += 1;
                if self.functions == 0 {
                    return Err(Error::syntax("'return' outside a function", pos));
                }
                let value = if matches!(self.peek(), Tok::Semicolon | Tok::RBrace) {
                    None
                } else {
                    Some(self.expression(0)?)
                };
                self.semicolon()?;
                Ok(Stmt::Return(value))
            }
            Tok::Break | Tok::Continue => {
                let is_break = self.next().tok == Tok::Break;
                if self.loops == 0 {
                    let word = if is_break { "break" } else { "continue" };
                    return Err(Error::syntax(format!("'{word}' outside a loop"), pos));
                }
                self.semicolon()?;
                Ok(if is_break {
                    Stmt::Break
                } else {
                    Stmt::Continue
                })
            }
            Tok::LBrace => Ok(Stmt::Block(self.block()?)),
            _ => {
                let expr = self.expression(0)?;
                self.semicolon()?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    /// Semicolons end statements, but are optional before a `}` or end of input.
    fn semicolon(&mut self) -> Result<(), Error> {
        if self.eat(&Tok::Semicolon) || matches!(self.peek(), Tok::RBrace | Tok::Eof) {
            Ok(())
        } else {
            Err(Error::syntax(
                format!("expected ';', found {}", describe(self.peek())),
                self.pos(),
            ))
        }
    }

    fn block(&mut self) -> Result<Vec<Stmt>, Error> {
        let open = self.expect(Tok::LBrace, "'{'")?;
        let mut body = Vec::new();
        while !matches!(self.peek(), Tok::RBrace) {
            if self.peek() == &Tok::Eof {
                return Err(Error::syntax("unclosed '{'", open));
            }
            body.push(self.statement()?);
        }
        self.i += 1;
        Ok(body)
    }

    fn loop_body(&mut self) -> Result<Vec<Stmt>, Error> {
        self.loops += 1;
        let body = self.block();
        self.loops -= 1;
        body
    }

    fn if_statement(&mut self) -> Result<Stmt, Error> {
        self.expect(Tok::If, "'if'")?;
        let cond = self.expression(0)?;
        let then = self.block()?;
        let otherwise = if self.eat(&Tok::Else) {
            if self.peek() == &Tok::If {
                Some(vec![self.if_statement()?]) // else if ... is sugar for else { if ... }
            } else {
                Some(self.block()?)
            }
        } else {
            None
        };
        Ok(Stmt::If(cond, then, otherwise))
    }

    fn function(&mut self, name: String, pos: Pos) -> Result<Rc<FunctionDef>, Error> {
        self.expect(Tok::LParen, "'(' to start the parameter list")?;
        let mut params = Vec::new();
        while !self.eat(&Tok::RParen) {
            let param_pos = self.pos();
            let param = self.ident("a parameter name")?;
            if params.contains(&param) {
                return Err(Error::syntax(
                    format!("duplicate parameter '{param}'"),
                    param_pos,
                ));
            }
            params.push(param);
            if !self.eat(&Tok::Comma) {
                self.expect(Tok::RParen, "',' or ')'")?;
                break;
            }
        }
        // A function body resets loop depth: `break` can't escape a function.
        let saved_loops = std::mem::replace(&mut self.loops, 0);
        self.functions += 1;
        let body = self.block();
        self.functions -= 1;
        self.loops = saved_loops;
        Ok(Rc::new(FunctionDef {
            name,
            params,
            body: body?,
            pos,
        }))
    }

    // ------------------------------------------------------------- expressions

    fn expression(&mut self, min_bp: u8) -> Result<Expr, Error> {
        let mut left = self.prefix()?;
        loop {
            let pos = self.pos();
            let (bp, op) = match self.peek() {
                Tok::Or => (OR, None),
                Tok::And => (AND, None),
                Tok::Eq => (EQUALITY, Some(BinOp::Eq)),
                Tok::NotEq => (EQUALITY, Some(BinOp::NotEq)),
                Tok::Lt => (COMPARISON, Some(BinOp::Lt)),
                Tok::Le => (COMPARISON, Some(BinOp::Le)),
                Tok::Gt => (COMPARISON, Some(BinOp::Gt)),
                Tok::Ge => (COMPARISON, Some(BinOp::Ge)),
                Tok::Plus => (TERM, Some(BinOp::Add)),
                Tok::Minus => (TERM, Some(BinOp::Sub)),
                Tok::Star => (FACTOR, Some(BinOp::Mul)),
                Tok::Slash => (FACTOR, Some(BinOp::Div)),
                Tok::Percent => (FACTOR, Some(BinOp::Rem)),
                Tok::LParen | Tok::LBracket => (POSTFIX, None),
                Tok::Assign => {
                    // Assignment is right-associative and binds loosest of all,
                    // so only take it when we're at the outermost level.
                    if min_bp > 0 {
                        break;
                    }
                    self.i += 1;
                    let value = self.expression(0)?;
                    left = match left {
                        Expr::Var(name, _) => Expr::Assign(name, Box::new(value), pos),
                        Expr::Index(target, index, _) => Expr::SetIndex {
                            target,
                            index,
                            value: Box::new(value),
                            pos,
                        },
                        _ => {
                            return Err(Error::syntax(
                                "can only assign to a variable or an index like a[i]",
                                pos,
                            ))
                        }
                    };
                    continue;
                }
                _ => break,
            };
            if bp <= min_bp {
                break; // left-associative: equal power stops here
            }
            match self.next().tok {
                Tok::LParen => {
                    let args = self.list_items(Tok::RParen)?;
                    left = Expr::Call(Box::new(left), args, pos);
                }
                Tok::LBracket => {
                    let index = self.expression(0)?;
                    self.expect(Tok::RBracket, "']'")?;
                    left = Expr::Index(Box::new(left), Box::new(index), pos);
                }
                Tok::And | Tok::Or => {
                    let is_and = bp == AND;
                    let right = self.expression(bp)?;
                    left = Expr::Logical {
                        left: Box::new(left),
                        is_and,
                        right: Box::new(right),
                    };
                }
                _ => {
                    let right = self.expression(bp)?;
                    left = Expr::Binary(Box::new(left), op.unwrap(), Box::new(right), pos);
                }
            }
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr, Error> {
        let Token { tok, pos } = self.next();
        Ok(match tok {
            Tok::Number(n) => Expr::Number(n),
            Tok::Str(s) => Expr::Str(s.into()),
            Tok::True => Expr::Bool(true),
            Tok::False => Expr::Bool(false),
            Tok::Nil => Expr::Nil,
            Tok::Ident(name) => Expr::Var(name, pos),
            Tok::Minus => Expr::Unary(UnOp::Neg, Box::new(self.expression(UNARY)?), pos),
            Tok::Not => Expr::Unary(UnOp::Not, Box::new(self.expression(UNARY)?), pos),
            Tok::LParen => {
                let inner = self.expression(0)?;
                self.expect(Tok::RParen, "')'")?;
                inner
            }
            Tok::LBracket => Expr::List(self.list_items(Tok::RBracket)?),
            Tok::LBrace => {
                let mut entries = Vec::new();
                while !self.eat(&Tok::RBrace) {
                    let key = self.expression(0)?;
                    self.expect(Tok::Colon, "':' between a key and its value")?;
                    entries.push((key, self.expression(0)?));
                    if !self.eat(&Tok::Comma) {
                        self.expect(Tok::RBrace, "',' or '}'")?;
                        break;
                    }
                }
                Expr::Map(entries, pos)
            }
            Tok::Fn => Expr::Function(self.function("<anonymous>".into(), pos)?),
            other => {
                return Err(Error::syntax(
                    format!("expected an expression, found {}", describe(&other)),
                    pos,
                ))
            }
        })
    }

    /// Comma-separated expressions up to `close` (trailing comma allowed).
    fn list_items(&mut self, close: Tok) -> Result<Vec<Expr>, Error> {
        let mut items = Vec::new();
        while !self.eat(&close) {
            items.push(self.expression(0)?);
            if !self.eat(&Tok::Comma) {
                let what = format!("',' or {}", describe(&close));
                self.expect(close, &what)?;
                break;
            }
        }
        Ok(items)
    }
}

fn describe(tok: &Tok) -> String {
    match tok {
        Tok::Number(n) => format!("number {n}"),
        Tok::Str(s) => format!("string {s:?}"),
        Tok::Ident(name) => format!("'{name}'"),
        Tok::Eof => "end of input".into(),
        other => {
            let text = match other {
                Tok::Let => "let",
                Tok::Fn => "fn",
                Tok::Return => "return",
                Tok::If => "if",
                Tok::Else => "else",
                Tok::While => "while",
                Tok::For => "for",
                Tok::In => "in",
                Tok::Break => "break",
                Tok::Continue => "continue",
                Tok::True => "true",
                Tok::False => "false",
                Tok::Nil => "nil",
                Tok::And => "and",
                Tok::Or => "or",
                Tok::Not => "not",
                Tok::LParen => "(",
                Tok::RParen => ")",
                Tok::LBrace => "{",
                Tok::RBrace => "}",
                Tok::LBracket => "[",
                Tok::RBracket => "]",
                Tok::Comma => ",",
                Tok::Colon => ":",
                Tok::Semicolon => ";",
                Tok::Plus => "+",
                Tok::Minus => "-",
                Tok::Star => "*",
                Tok::Slash => "/",
                Tok::Percent => "%",
                Tok::Assign => "=",
                Tok::Eq => "==",
                Tok::NotEq => "!=",
                Tok::Lt => "<",
                Tok::Le => "<=",
                Tok::Gt => ">",
                Tok::Ge => ">=",
                Tok::Number(_) | Tok::Str(_) | Tok::Ident(_) | Tok::Eof => unreachable!(),
            };
            format!("'{text}'")
        }
    }
}
