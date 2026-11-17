//! The syntax tree the parser builds and the interpreter walks.

use std::rc::Rc;

use crate::error::Pos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug)]
pub enum Expr {
    Number(f64),
    Str(Rc<str>),
    Bool(bool),
    Nil,
    Var(String, Pos),
    Assign(String, Box<Expr>, Pos),
    SetIndex {
        target: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
        pos: Pos,
    },
    Unary(UnOp, Box<Expr>, Pos),
    Binary(Box<Expr>, BinOp, Box<Expr>, Pos),
    /// `and` / `or`, which short-circuit, so they're not ordinary binary ops.
    Logical {
        left: Box<Expr>,
        is_and: bool,
        right: Box<Expr>,
    },
    Call(Box<Expr>, Vec<Expr>, Pos),
    Index(Box<Expr>, Box<Expr>, Pos),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>, Pos),
    Function(Rc<FunctionDef>),
}

#[derive(Debug)]
pub struct FunctionDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

#[derive(Debug)]
pub enum Stmt {
    Let(String, Expr),
    Expr(Expr),
    Block(Vec<Stmt>),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>, Pos),
    Return(Option<Expr>),
    Break,
    Continue,
    Fn(Rc<FunctionDef>),
}
