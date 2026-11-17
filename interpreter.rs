//! A tree-walking interpreter: evaluate the syntax tree directly.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::rc::Rc;

use crate::ast::{BinOp, Expr, FunctionDef, Stmt, UnOp};
use crate::builtins;
use crate::error::{Error, Pos};
use crate::value::{Closure, Value};

/// Pebble calls nest Rust calls, so cap the depth and report a clean error
/// instead of letting the host stack overflow.
pub const MAX_CALL_DEPTH: usize = 5_000;

/// A scope: its own variables plus a link to the enclosing scope.
#[derive(Default)]
pub struct Env {
    vars: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Env>>>,
}

impl Env {
    pub fn child(parent: &Rc<RefCell<Env>>) -> Rc<RefCell<Env>> {
        Rc::new(RefCell::new(Env {
            vars: HashMap::new(),
            parent: Some(parent.clone()),
        }))
    }

    pub fn define(&mut self, name: &str, value: Value) {
        self.vars.insert(name.to_string(), value);
    }

    fn get(&self, name: &str) -> Option<Value> {
        match self.vars.get(name) {
            Some(v) => Some(v.clone()),
            None => self.parent.as_ref()?.borrow().get(name),
        }
    }

    /// Assign to the nearest scope that already has `name`.
    fn set(&mut self, name: &str, value: Value) -> bool {
        if let Some(slot) = self.vars.get_mut(name) {
            *slot = value;
            true
        } else if let Some(parent) = &self.parent {
            parent.borrow_mut().set(name, value)
        } else {
            false
        }
    }
}

/// Non-local control flow travels up through `Err` alongside real errors, which
/// lets `?` unwind out of nested statements in one step.
pub enum Flow {
    Return(Value),
    Break,
    Continue,
    Error(Error),
}

impl From<Error> for Flow {
    fn from(e: Error) -> Self {
        Flow::Error(e)
    }
}

type Exec<T = ()> = Result<T, Flow>;

pub struct Interpreter {
    pub globals: Rc<RefCell<Env>>,
    pub out: Box<dyn Write>,
    depth: usize,
}

fn err<T>(message: impl Into<String>, pos: Pos) -> Exec<T> {
    Err(Flow::Error(Error::runtime(message, pos)))
}

impl Interpreter {
    pub fn new(out: Box<dyn Write>) -> Self {
        let globals = Rc::new(RefCell::new(Env::default()));
        builtins::install(&mut globals.borrow_mut());
        Interpreter {
            globals,
            out,
            depth: 0,
        }
    }

    /// Run a program. Returns the value of a trailing expression statement, which
    /// the REPL prints.
    pub fn run(&mut self, program: &[Stmt]) -> Result<Option<Value>, Error> {
        let env = self.globals.clone();
        let mut last = None;
        for stmt in program {
            last = None;
            let result = match stmt {
                Stmt::Expr(e) => self.eval(e, &env).map(|v| last = Some(v)),
                other => self.exec(other, &env),
            };
            match result {
                Ok(()) => {}
                Err(Flow::Error(e)) => return Err(e),
                // The parser rejects return/break/continue in the wrong places.
                Err(_) => unreachable!("control flow escaped to top level"),
            }
        }
        let _ = self.out.flush();
        Ok(last)
    }

    fn exec_block(&mut self, body: &[Stmt], env: &Rc<RefCell<Env>>) -> Exec {
        for stmt in body {
            self.exec(stmt, env)?;
        }
        Ok(())
    }

    fn exec(&mut self, stmt: &Stmt, env: &Rc<RefCell<Env>>) -> Exec {
        match stmt {
            Stmt::Let(name, value) => {
                let v = self.eval(value, env)?;
                env.borrow_mut().define(name, v);
            }
            Stmt::Expr(e) => {
                self.eval(e, env)?;
            }
            Stmt::Block(body) => self.exec_block(body, &Env::child(env))?,
            Stmt::If(cond, then, otherwise) => {
                if self.eval(cond, env)?.truthy() {
                    self.exec_block(then, &Env::child(env))?;
                } else if let Some(otherwise) = otherwise {
                    self.exec_block(otherwise, &Env::child(env))?;
                }
            }
            Stmt::While(cond, body) => {
                while self.eval(cond, env)?.truthy() {
                    match self.exec_block(body, &Env::child(env)) {
                        Ok(()) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
            }
            Stmt::For(var, iterable, body, pos) => {
                let items: Vec<Value> = match self.eval(iterable, env)? {
                    Value::List(list) => list.borrow().clone(), // snapshot: mutating while looping is safe
                    Value::Map(map) => map
                        .borrow()
                        .keys()
                        .map(|k| Value::str(k.as_str()))
                        .collect(),
                    Value::Str(s) => s.chars().map(|c| Value::str(c.to_string())).collect(),
                    other => return err(format!("can't loop over a {}", other.type_name()), *pos),
                };
                for item in items {
                    let scope = Env::child(env);
                    scope.borrow_mut().define(var, item);
                    match self.exec_block(body, &scope) {
                        Ok(()) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
            }
            Stmt::Return(value) => {
                let v = match value {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Nil,
                };
                return Err(Flow::Return(v));
            }
            Stmt::Break => return Err(Flow::Break),
            Stmt::Continue => return Err(Flow::Continue),
            Stmt::Fn(def) => {
                let closure = self.make_closure(def, env);
                env.borrow_mut().define(&def.name, closure);
            }
        }
        Ok(())
    }

    fn make_closure(&self, def: &Rc<FunctionDef>, env: &Rc<RefCell<Env>>) -> Value {
        Value::Closure(Rc::new(Closure {
            def: def.clone(),
            env: env.clone(),
        }))
    }

    pub fn eval(&mut self, expr: &Expr, env: &Rc<RefCell<Env>>) -> Exec<Value> {
        Ok(match expr {
            Expr::Number(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Bool(b) => Value::Bool(*b),
            Expr::Nil => Value::Nil,
            Expr::Var(name, pos) => match env.borrow().get(name) {
                Some(v) => v,
                None => return err(format!("undefined variable '{name}'"), *pos),
            },
            Expr::Assign(name, value, pos) => {
                let v = self.eval(value, env)?;
                if !env.borrow_mut().set(name, v.clone()) {
                    return err(
                        format!("assignment to undeclared variable '{name}' (use 'let' first)"),
                        *pos,
                    );
                }
                v
            }
            Expr::SetIndex {
                target,
                index,
                value,
                pos,
            } => {
                let target = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                let v = self.eval(value, env)?;
                match (&target, &index) {
                    (Value::List(list), Value::Num(_)) => {
                        let mut list = list.borrow_mut();
                        let i = list_index(&index, list.len(), *pos)?;
                        list[i] = v.clone();
                    }
                    (Value::Map(map), Value::Str(key)) => {
                        map.borrow_mut().insert(key.to_string(), v.clone());
                    }
                    _ => {
                        return err(
                            format!(
                                "can't set {} index on a {}",
                                index.type_name(),
                                target.type_name()
                            ),
                            *pos,
                        )
                    }
                }
                v
            }
            Expr::Unary(op, operand, pos) => {
                let v = self.eval(operand, env)?;
                match (op, v) {
                    (UnOp::Neg, Value::Num(n)) => Value::Num(-n),
                    (UnOp::Not, v) => Value::Bool(!v.truthy()),
                    (UnOp::Neg, v) => {
                        return err(format!("can't negate a {}", v.type_name()), *pos)
                    }
                }
            }
            Expr::Logical {
                left,
                is_and,
                right,
            } => {
                let l = self.eval(left, env)?;
                if l.truthy() != *is_and {
                    l // short-circuit: `false and x` and `true or x` skip x
                } else {
                    self.eval(right, env)?
                }
            }
            Expr::Binary(l, op, r, pos) => {
                let a = self.eval(l, env)?;
                let b = self.eval(r, env)?;
                binary(*op, a, b, *pos)?
            }
            Expr::Index(target, index, pos) => {
                let target = self.eval(target, env)?;
                let index = self.eval(index, env)?;
                match (&target, &index) {
                    (Value::List(list), Value::Num(_)) => {
                        let list = list.borrow();
                        list[list_index(&index, list.len(), *pos)?].clone()
                    }
                    (Value::Str(s), Value::Num(_)) => {
                        let chars: Vec<char> = s.chars().collect();
                        Value::str(chars[list_index(&index, chars.len(), *pos)?].to_string())
                    }
                    (Value::Map(map), Value::Str(key)) => {
                        map.borrow().get(&**key).cloned().unwrap_or(Value::Nil)
                    }
                    _ => {
                        return err(
                            format!(
                                "can't index a {} with a {}",
                                target.type_name(),
                                index.type_name()
                            ),
                            *pos,
                        )
                    }
                }
            }
            Expr::List(items) => {
                let values = items
                    .iter()
                    .map(|e| self.eval(e, env))
                    .collect::<Exec<Vec<_>>>()?;
                Value::list(values)
            }
            Expr::Map(entries, pos) => {
                let mut map = BTreeMap::new();
                for (k, v) in entries {
                    let key = match self.eval(k, env)? {
                        Value::Str(s) => s.to_string(),
                        other => {
                            return err(
                                format!("map keys must be strings, not {}", other.type_name()),
                                *pos,
                            )
                        }
                    };
                    let value = self.eval(v, env)?;
                    map.insert(key, value);
                }
                Value::Map(Rc::new(RefCell::new(map)))
            }
            Expr::Function(def) => self.make_closure(def, env),
            Expr::Call(callee, args, pos) => {
                let f = self.eval(callee, env)?;
                let args = args
                    .iter()
                    .map(|a| self.eval(a, env))
                    .collect::<Exec<Vec<_>>>()?;
                self.call(f, args, *pos)?
            }
        })
    }

    pub fn call(&mut self, f: Value, args: Vec<Value>, pos: Pos) -> Exec<Value> {
        match f {
            Value::Native(name, native) => {
                native(self, args).or_else(|msg| err(format!("{name}: {msg}"), pos))
            }
            Value::Closure(closure) => {
                let def = &closure.def;
                if args.len() != def.params.len() {
                    let s = if def.params.len() == 1 { "" } else { "s" };
                    return err(
                        format!(
                            "{} takes {} argument{s} but got {}",
                            def.name,
                            def.params.len(),
                            args.len()
                        ),
                        pos,
                    );
                }
                if self.depth >= MAX_CALL_DEPTH {
                    return err(
                        format!("stack overflow (more than {MAX_CALL_DEPTH} nested calls)"),
                        pos,
                    );
                }
                let scope = Env::child(&closure.env);
                for (param, arg) in def.params.iter().zip(args) {
                    scope.borrow_mut().define(param, arg);
                }
                self.depth += 1;
                let result = self.exec_block(&def.body, &scope);
                self.depth -= 1;
                match result {
                    Ok(()) => Ok(Value::Nil),
                    Err(Flow::Return(v)) => Ok(v),
                    Err(Flow::Error(mut e)) => {
                        if e.trace.len() < 20 {
                            e.trace.push((def.name.clone(), pos));
                        }
                        Err(Flow::Error(e))
                    }
                    Err(Flow::Break | Flow::Continue) => {
                        unreachable!("parser keeps break inside loops")
                    }
                }
            }
            other => err(format!("can't call a {}", other.type_name()), pos),
        }
    }
}

/// Lists accept negative indices counting from the end, like Python.
fn list_index(index: &Value, len: usize, pos: Pos) -> Exec<usize> {
    let Value::Num(n) = index else { unreachable!() };
    if n.fract() != 0.0 {
        return err(format!("index {n} is not a whole number"), pos);
    }
    let i = if *n < 0.0 { len as f64 + n } else { *n };
    if i < 0.0 || i >= len as f64 {
        return err(
            format!(
                "index {} out of range for length {len}",
                crate::value::format_number(*n)
            ),
            pos,
        );
    }
    Ok(i as usize)
}

fn binary(op: BinOp, a: Value, b: Value, pos: Pos) -> Exec<Value> {
    use Value::*;
    Ok(match (op, &a, &b) {
        (BinOp::Eq, _, _) => Bool(a == b),
        (BinOp::NotEq, _, _) => Bool(a != b),
        (BinOp::Add, Num(x), Num(y)) => Num(x + y),
        // Strings concatenate with anything, so "n = " + 3 just works.
        (BinOp::Add, Str(_), _) | (BinOp::Add, _, Str(_)) => Value::str(format!("{a}{b}")),
        (BinOp::Add, List(x), List(y)) => {
            let mut items = x.borrow().clone();
            items.extend(y.borrow().iter().cloned());
            Value::list(items)
        }
        (BinOp::Sub, Num(x), Num(y)) => Num(x - y),
        (BinOp::Mul, Num(x), Num(y)) => Num(x * y),
        (BinOp::Mul, Str(s), Num(n)) | (BinOp::Mul, Num(n), Str(s)) if *n >= 0.0 => {
            Value::str(s.repeat(*n as usize))
        }
        (BinOp::Div, Num(_), Num(y)) if *y == 0.0 => return err("division by zero", pos),
        (BinOp::Div, Num(x), Num(y)) => Num(x / y),
        (BinOp::Rem, Num(_), Num(y)) if *y == 0.0 => return err("division by zero", pos),
        (BinOp::Rem, Num(x), Num(y)) => Num(x.rem_euclid(*y)),
        (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Num(x), Num(y)) => {
            Bool(compare(op, x.partial_cmp(y)))
        }
        (BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, Str(x), Str(y)) => {
            Bool(compare(op, x.partial_cmp(y)))
        }
        _ => {
            let symbol = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Rem => "%",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
                BinOp::Eq | BinOp::NotEq => unreachable!(),
            };
            return err(
                format!(
                    "can't apply '{symbol}' to {} and {}",
                    a.type_name(),
                    b.type_name()
                ),
                pos,
            );
        }
    })
}

fn compare(op: BinOp, ord: Option<std::cmp::Ordering>) -> bool {
    use std::cmp::Ordering::*;
    match (op, ord) {
        (_, None) => false, // NaN
        (BinOp::Lt, Some(o)) => o == Less,
        (BinOp::Le, Some(o)) => o != Greater,
        (BinOp::Gt, Some(o)) => o == Greater,
        (BinOp::Ge, Some(o)) => o != Less,
        _ => unreachable!(),
    }
}
