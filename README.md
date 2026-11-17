# pebble

**A small scripting language written in Rust, with closures, lists, maps, a REPL, and error messages that point at the problem.**

```js
fn make_counter() {
  let count = 0;
  return fn() {
    count = count + 1;
    return count;
  };
}

let a = make_counter();
let b = make_counter();
a(); a();
print("a:", a(), " b:", b());     // a: 3  b: 1

let squares = map(range(1, 11), fn(n) { return n * n; });
print(filter(squares, fn(n) { return n % 2 == 0; }));   // [4, 16, 36, 64, 100]
```

The whole pipeline (lexer, Pratt parser, tree-walking interpreter, standard
library) is under 1,800 lines with **zero dependencies**.

## Try it

```bash
cargo run --release -- examples/mandelbrot.pb
cargo run --release                      # REPL
cargo run --release -- -e 'print(2 * 21);'
```

```text
$ cargo run --release -- examples/mandelbrot.pb
                       ..............................,::.............
                   ..................................,,%:,,;,............
                ....................................,,,:-=:,,,..............
              ...................................,,:+=-@@%%;;,,.............
            ................................,,,,,,,,+@@@@@@%:,,,,,..........
          ...............................,,+==:::%;%-=*@@@+-;@:=;,,,,:;,....
         .............................,,,,,:+@@+@@@@@@@@@@@@@@@@@-=@#%%,....
       ............................,,,,,:;;-#@@@@@@@@@@@@@@@@@@@@@@@%:,,,...
       ................,,:,,,,,,,,,,,,,,:-%@@@@@@@@@@@@@@@@@@@@@@@@@@=::;...
      ................,,,:+-;::-%;%::::;=@@@@@@@@@@@@@@@@@@@@@@@@@@@@@%%,,..
     ................,,,::;%@@@@@@@@*;;=@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@=,...
     ..........,,,,,:::::=@@@@@@@@@@@@*%@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@#,,...
     ..,,,,:,,,,,,::;-%@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@:,,....
     ..,,,,:,,,,,,::;-%@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@:,,....
     ..........,,,,,:::::=@@@@@@@@@@@@*%@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@#,,...
     ...
```

### Errors that help

```text
$ cargo run -- examples/errors.pb
average: 84
runtime error: division by zero
  --> line 3, column 18
   |
 3 |   return sum(xs) / len(xs);
   |                  ^
  in average (called at line 7)
  in report (called at line 11)
```

Syntax errors work the same way: `expected ',' or ')', found number 2`,
`'break' outside a loop`, `unterminated string`, and so on, each with a line and column.

## The language

| Feature | Example |
| --- | --- |
| Values | `nil`, `true`, `3.14`, `"text"`, `[1, 2]`, `{"key": "value"}`, functions |
| Variables | `let x = 1;` then `x = x + 1;` (assigning to an undeclared name is an error) |
| Operators | `+ - * / %`, `== != < <= > >=`, `and or not` (short-circuiting) |
| Control flow | `if / else if / else`, `while`, `for x in list_or_map_or_string`, `break`, `continue` |
| Functions | `fn name(a, b) { ... }`, anonymous `fn(x) { ... }`, closures, recursion |
| Indexing | `xs[0]`, `xs[-1]`, `m["key"]`, `"str"[1]`, and assignment `xs[i] = v` |
| Builtins (Rust) | `print len push pop str num type range keys sort join split sqrt floor abs clock assert` |
| Prelude (Pebble) | `map filter reduce sum contains reverse` |

Some deliberate choices:
- **Only `nil` and `false` are falsy.** `0` and `""` are true, as in Lua and Ruby.
- **`+` with a string concatenates anything**, so `"n = " + 3` works.
- **Lists and maps are shared references.** `let b = a; push(b, 1);` changes `a` too,
  and `==` compares them structurally.
- **Semicolons** end statements, but are optional before `}`.

## How it works

```
source ──lexer.rs──▶ tokens ──parser.rs──▶ syntax tree ──interpreter.rs──▶ output
```

- **[`lexer.rs`](src/lexer.rs)** turns characters into tokens and tracks line and
  column for every one, which is what lets errors point at exact spots.
- **[`parser.rs`](src/parser.rs)** parses statements by recursive descent and
  expressions with a **Pratt parser**. Each operator has a *binding power*, and a
  single loop, `expression(min_bp)`, keeps consuming operators that bind tighter
  than `min_bp`. That one function handles all precedence levels and
  associativity, plus calls `f(x)` and indexing `a[i]` as postfix operators. The
  parser also rejects `break` outside loops and `return` outside functions, so the
  interpreter never has to.
- **[`interpreter.rs`](src/interpreter.rs)** walks the tree. Scopes are a chain of
  `Rc<RefCell<Env>>`. A closure is just a function plus a pointer to the scope it
  was created in, which is why `count` survives after `make_counter` returns.
  `return`, `break` and `continue` travel up the Rust call stack as `Err(Flow::...)`
  values next to real errors, so `?` unwinds through nested blocks in one step.
- **[`builtins.rs`](src/builtins.rs)** has the native functions, plus a *prelude
  written in Pebble*. `map`, `filter` and `reduce` are ordinary Pebble functions
  loaded at startup.

Pebble recursion becomes Rust recursion, so the binary runs the interpreter on a
512 MB thread and caps call depth at 5,000. Runaway recursion becomes
`runtime error: stack overflow` rather than a crash.

## Tests

```bash
cargo test
```

The suite covers arithmetic and precedence, scoping, closures, control flow,
shared mutable collections, every syntax and runtime error message (with
positions and call traces), deep recursion, and every program in
[`examples/`](examples/).

## Ideas for extending it

A bytecode compiler and VM (the classic next step, and much faster), a mark-and-sweep
garbage collector (closures that capture their own scope currently leak via `Rc`
cycles), string interpolation, or modules.

## License

MIT
