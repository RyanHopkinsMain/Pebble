use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

/// Collects `print` output so tests can inspect it.
#[derive(Clone, Default)]
struct Captured(Rc<RefCell<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Runs a program on a large stack, like the `pebble` binary does: unoptimized
/// test builds use far more stack per Pebble call than release builds.
fn run(src: &str) -> Result<String, pebble::Error> {
    let src = src.to_string();
    std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(move || {
            let out = Captured::default();
            pebble::run(&src, Box::new(out.clone()))?;
            let bytes = out.0.borrow().clone();
            Ok(String::from_utf8(bytes).unwrap())
        })
        .unwrap()
        .join()
        .unwrap()
}

fn output(src: &str) -> String {
    run(src).unwrap_or_else(|e| panic!("program failed: {}\n{}", e, e.render(src)))
}

fn error(src: &str) -> String {
    match run(src) {
        Ok(out) => panic!("expected an error, got output {out:?}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(
        output("print(1 + 2 * 3, (1 + 2) * 3, 10 - 4 - 3, 2 * 3 % 4, -2 * 3);"),
        "7 9 3 2 -6\n"
    );
    assert_eq!(
        output("print(7 / 2, 0.1 + 0.2 == 0.3, -7 % 3, 1_000_000);"),
        "3.5 false 2 1000000\n"
    );
    assert_eq!(
        output("print(1 < 2 and 2 < 3, not true or false, 1 == 1 != false);"),
        "true false true\n"
    );
}

#[test]
fn strings() {
    assert_eq!(
        output(r#"print("ab" + "cd", "n=" + 4, "ha" * 3, len("héllo"), "abc"[-1]);"#),
        "abcd n=4 hahaha 5 c\n"
    );
    assert_eq!(
        output(r#"print(split("a,b,c", ","), join([1, 2, 3], "-"), "a" < "b");"#),
        "[\"a\", \"b\", \"c\"] 1-2-3 true\n"
    );
    assert_eq!(
        output(r#"print("tab\there", "quote\"d");"#),
        "tab\there quote\"d\n"
    );
}

#[test]
fn variables_and_scope() {
    let src = "
        let x = 1;
        { let x = 2; print(x); }
        print(x);
        if true { x = 3; }
        print(x);";
    assert_eq!(output(src), "2\n1\n3\n");
    assert!(error("y = 5;").contains("undeclared variable 'y'"));
    assert!(error("print(nope);").contains("undefined variable 'nope'"));
}

#[test]
fn control_flow() {
    let src = "
        let total = 0;
        let i = 0;
        while true {
            i = i + 1;
            if i > 10 { break; }
            if i % 2 == 0 { continue; }
            total = total + i;
        }
        print(total);
        for c in \"hey\" { print(c); }
        for k in {\"b\": 1, \"a\": 2} { print(k); }";
    assert_eq!(output(src), "25\nh\ne\ny\na\nb\n");
}

#[test]
fn closures_capture_their_environment() {
    let src = "
        fn make_counter() {
            let n = 0;
            return fn() { n = n + 1; return n; };
        }
        let a = make_counter();
        let b = make_counter();
        a(); a();
        print(a(), b());";
    assert_eq!(output(src), "3 1\n");
}

#[test]
fn recursion_and_higher_order_functions() {
    let src = "
        fn fact(n) { if n <= 1 { return 1; } return n * fact(n - 1); }
        print(fact(10));
        print(map([1, 2, 3], fn(x) { return x * x; }));
        print(filter(range(10), fn(x) { return x % 3 == 0; }));
        print(reduce([1, 2, 3, 4], fn(a, b) { return a * b; }, 1), sum(range(101)));
        print(reverse([1, 2, 3]), contains([1, 2], 2), sort([3, 1, 2]));";
    assert_eq!(
        output(src),
        "3628800\n[1, 4, 9]\n[0, 3, 6, 9]\n24 5050\n[3, 2, 1] true [1, 2, 3]\n"
    );
}

#[test]
fn lists_and_maps_are_shared_and_mutable() {
    let src = r#"
        let xs = [1, 2, 3];
        let ys = xs;
        push(ys, 4);
        xs[0] = 10;
        print(xs, len(ys));
        print(pop(xs), xs[-1], len(ys));
        let m = {"a": 1};
        m["b"] = [1, {"c": nil}];
        print(m, m["missing"], keys(m), [1, [2]] == [1, [2]]);
    "#;
    assert_eq!(
        output(src),
        "[10, 2, 3, 4] 4\n4 3 3\n{\"a\": 1, \"b\": [1, {\"c\": nil}]} nil [\"a\", \"b\"] true\n"
    );
}

#[test]
fn deep_recursion_is_an_error_not_a_crash() {
    let ok =
        output("fn down(n) { if n == 0 { return 0; } return down(n - 1); } print(down(4000));");
    assert_eq!(ok, "0\n");
    let err = error("fn forever(n) { return forever(n + 1); } forever(0);");
    assert!(err.contains("stack overflow"), "{err}");
}

#[test]
fn runtime_errors_have_positions_and_traces() {
    let src = "fn divide(a, b) {\n  return a / b;\n}\nprint(divide(1, 0));";
    let e = pebble::run(src, Box::new(Captured::default())).unwrap_err();
    let report = e.render(src);
    assert!(
        report.contains("runtime error: division by zero"),
        "{report}"
    );
    assert!(report.contains("--> line 2, column 12"), "{report}");
    assert!(report.contains(" 2 |   return a / b;"), "{report}");
    assert!(report.contains("in divide (called at line 4)"), "{report}");

    assert!(error("[1, 2][5];").contains("index 5 out of range for length 2"));
    assert!(error("1 + nil;").contains("can't apply '+' to number and nil"));
    assert!(error("fn f(a) {} f(1, 2);").contains("f takes 1 argument but got 2"));
    assert!(error("5();").contains("can't call a number"));
    assert!(
        error("assert(1 == 2, \"math is broken\");").contains("assertion failed: math is broken")
    );
}

#[test]
fn syntax_errors_are_reported_with_positions() {
    let cases = [
        (
            "let = 4;",
            "expected a variable name, found '=' (line 1, column 5)",
        ),
        ("print(1 2);", "expected ',' or ')', found number 2"),
        ("break;", "'break' outside a loop"),
        ("return 1;", "'return' outside a function"),
        (
            "fn f() { while true { fn g() { break; } } }",
            "'break' outside a loop",
        ),
        ("1 = 2;", "can only assign to a variable"),
        ("let s = \"open", "unterminated string"),
        ("let x = 3 $ 4;", "unexpected character '$'"),
        ("if true { print(1);", "unclosed '{'"),
        ("fn f(a, a) {}", "duplicate parameter 'a'"),
    ];
    for (src, want) in cases {
        let got = error(src);
        assert!(got.contains(want), "{src:?}\n  got:  {got}\n  want: {want}");
    }
}

#[test]
fn every_example_runs() {
    for entry in std::fs::read_dir("examples").unwrap() {
        let path = entry.unwrap().path();
        let src = std::fs::read_to_string(&path).unwrap();
        let result = run(&src);
        if path.file_name().unwrap() == "errors.pb" {
            assert!(result.is_err(), "errors.pb should fail");
        } else {
            assert!(
                result.is_ok(),
                "{} failed: {:?}",
                path.display(),
                result.err()
            );
        }
    }
}
