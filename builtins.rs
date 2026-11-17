//! Functions implemented in Rust. Higher-order helpers (map, filter, reduce...)
//! are written in Pebble itself; see `PRELUDE`.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::interpreter::{Env, Interpreter};
use crate::value::{NativeFn, Value};

type R = Result<Value, String>;

pub fn install(env: &mut Env) {
    let natives: &[(&'static str, NativeFn)] = &[
        ("print", print),
        ("len", len),
        ("push", push),
        ("pop", pop),
        ("str", to_str),
        ("num", to_num),
        ("type", type_of),
        ("range", range),
        ("keys", keys),
        ("sort", sort),
        ("join", join),
        ("split", split),
        ("sqrt", |_, a| math(a, f64::sqrt)),
        ("floor", |_, a| math(a, f64::floor)),
        ("abs", |_, a| math(a, f64::abs)),
        ("clock", clock),
        ("assert", assert),
    ];
    for (name, f) in natives {
        env.define(name, Value::Native(name, *f));
    }
}

/// The rest of the standard library, in Pebble.
pub const PRELUDE: &str = r#"
fn map(list, f) {
  let out = [];
  for x in list { push(out, f(x)); }
  return out;
}

fn filter(list, keep) {
  let out = [];
  for x in list { if keep(x) { push(out, x); } }
  return out;
}

fn reduce(list, f, acc) {
  for x in list { acc = f(acc, x); }
  return acc;
}

fn sum(list) { return reduce(list, fn(a, b) { return a + b; }, 0); }

fn contains(list, item) {
  for x in list { if x == item { return true; } }
  return false;
}

fn reverse(list) {
  let out = [];
  let i = len(list) - 1;
  while i >= 0 { push(out, list[i]); i = i - 1; }
  return out;
}
"#;

fn arity(args: &[Value], n: usize) -> Result<(), String> {
    if args.len() == n {
        Ok(())
    } else {
        Err(format!(
            "expected {n} argument{}, got {}",
            if n == 1 { "" } else { "s" },
            args.len()
        ))
    }
}

fn print(interp: &mut Interpreter, args: Vec<Value>) -> R {
    let line: Vec<String> = args.iter().map(Value::to_string).collect();
    writeln!(interp.out, "{}", line.join(" ")).map_err(|e| e.to_string())?;
    Ok(Value::Nil)
}

fn len(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    Ok(Value::Num(match &args[0] {
        Value::Str(s) => s.chars().count(),
        Value::List(l) => l.borrow().len(),
        Value::Map(m) => m.borrow().len(),
        other => return Err(format!("a {} has no length", other.type_name())),
    } as f64))
}

fn push(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 2)?;
    match &args[0] {
        Value::List(l) => {
            l.borrow_mut().push(args[1].clone());
            Ok(Value::Nil)
        }
        other => Err(format!("can't push onto a {}", other.type_name())),
    }
}

fn pop(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    match &args[0] {
        Value::List(l) => l
            .borrow_mut()
            .pop()
            .ok_or_else(|| "pop from an empty list".to_string()),
        other => Err(format!("can't pop from a {}", other.type_name())),
    }
}

fn to_str(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    Ok(Value::str(args[0].to_string()))
}

fn to_num(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    match &args[0] {
        Value::Num(n) => Ok(Value::Num(*n)),
        Value::Str(s) => Ok(s.trim().parse().map(Value::Num).unwrap_or(Value::Nil)),
        other => Err(format!("can't convert a {} to a number", other.type_name())),
    }
}

fn type_of(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    Ok(Value::str(args[0].type_name()))
}

fn range(_: &mut Interpreter, args: Vec<Value>) -> R {
    let (start, end) = match args.as_slice() {
        [Value::Num(end)] => (0.0, *end),
        [Value::Num(start), Value::Num(end)] => (*start, *end),
        _ => return Err("expected range(end) or range(start, end) with numbers".into()),
    };
    if end - start > 10_000_000.0 {
        return Err("range is too large".into());
    }
    let mut items = Vec::new();
    let mut i = start;
    while i < end {
        items.push(Value::Num(i));
        i += 1.0;
    }
    Ok(Value::list(items))
}

fn keys(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    match &args[0] {
        Value::Map(m) => Ok(Value::list(
            m.borrow().keys().map(|k| Value::str(k.as_str())).collect(),
        )),
        other => Err(format!("a {} has no keys", other.type_name())),
    }
}

fn sort(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 1)?;
    let Value::List(list) = &args[0] else {
        return Err(format!("can't sort a {}", args[0].type_name()));
    };
    let mut items = list.borrow().clone();
    if items.iter().all(|v| matches!(v, Value::Num(_))) {
        items.sort_by(|a, b| match (a, b) {
            (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
            _ => unreachable!(),
        });
    } else if items.iter().all(|v| matches!(v, Value::Str(_))) {
        items.sort_by(|a, b| match (a, b) {
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            _ => unreachable!(),
        });
    } else {
        return Err("can only sort a list of all numbers or all strings".into());
    }
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn join(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 2)?;
    match (&args[0], &args[1]) {
        (Value::List(l), Value::Str(sep)) => Ok(Value::str(
            l.borrow()
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(sep),
        )),
        _ => Err("expected join(list, separator)".into()),
    }
}

fn split(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Str(s), Value::Str(sep)) if sep.is_empty() => Ok(Value::list(
            s.chars().map(|c| Value::str(c.to_string())).collect(),
        )),
        (Value::Str(s), Value::Str(sep)) => {
            Ok(Value::list(s.split(&**sep).map(Value::str).collect()))
        }
        _ => Err("expected split(string, separator)".into()),
    }
}

fn math(args: Vec<Value>, f: fn(f64) -> f64) -> R {
    arity(&args, 1)?;
    match &args[0] {
        Value::Num(n) => Ok(Value::Num(f(*n))),
        other => Err(format!("expected a number, got a {}", other.type_name())),
    }
}

fn clock(_: &mut Interpreter, args: Vec<Value>) -> R {
    arity(&args, 0)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(Value::Num(now.as_secs_f64()))
}

fn assert(_: &mut Interpreter, args: Vec<Value>) -> R {
    match args.as_slice() {
        [cond] if cond.truthy() => Ok(Value::Nil),
        [cond, _] if cond.truthy() => Ok(Value::Nil),
        [_] => Err("assertion failed".into()),
        [_, message] => Err(format!("assertion failed: {message}")),
        _ => Err("expected assert(condition) or assert(condition, message)".into()),
    }
}
