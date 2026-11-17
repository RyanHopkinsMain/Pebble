//! Runtime values.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::FunctionDef;
use crate::interpreter::Env;

pub type NativeFn = fn(&mut crate::interpreter::Interpreter, Vec<Value>) -> Result<Value, String>;

#[derive(Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Num(f64),
    Str(Rc<str>),
    /// Lists and maps are shared and mutable, like in Python or JavaScript.
    List(Rc<RefCell<Vec<Value>>>),
    Map(Rc<RefCell<BTreeMap<String, Value>>>),
    Closure(Rc<Closure>),
    Native(&'static str, NativeFn),
}

/// A function plus the environment it was created in. Capturing `env` is what
/// makes closures work: a counter's `n` lives on after `make_counter` returns.
pub struct Closure {
    pub def: Rc<FunctionDef>,
    pub env: Rc<RefCell<Env>>,
}

impl Value {
    pub fn list(items: Vec<Value>) -> Value {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn str(s: impl Into<Rc<str>>) -> Value {
        Value::Str(s.into())
    }

    /// Only `nil` and `false` are falsy (as in Lua and Ruby); 0 and "" are true.
    pub fn truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Num(_) => "number",
            Value::Str(_) => "string",
            Value::List(_) => "list",
            Value::Map(_) => "map",
            Value::Closure(_) | Value::Native(..) => "function",
        }
    }

    /// The form used inside containers and the REPL: strings get quotes.
    pub fn repr(&self) -> String {
        match self {
            Value::Str(s) => format!("{s:?}"),
            other => other.to_string(),
        }
    }
}

impl PartialEq for Value {
    /// Structural equality for lists and maps; identity for functions.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            (Value::Native(a, _), Value::Native(b, _)) => a == b,
            _ => false,
        }
    }
}

pub fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e16 {
        format!("{}", n as i64) // 3 rather than 3.0
    } else {
        format!("{n}")
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(n) => write!(f, "{}", format_number(*n)),
            Value::Str(s) => write!(f, "{s}"),
            Value::List(items) => {
                let items = items.try_borrow().map_err(|_| fmt::Error)?;
                let parts: Vec<String> = items.iter().map(Value::repr).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Map(map) => {
                let map = map.try_borrow().map_err(|_| fmt::Error)?;
                let parts: Vec<String> = map
                    .iter()
                    .map(|(k, v)| format!("{k:?}: {}", v.repr()))
                    .collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            Value::Closure(c) => write!(f, "<fn {}>", c.def.name),
            Value::Native(name, _) => write!(f, "<builtin {name}>"),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.repr())
    }
}
