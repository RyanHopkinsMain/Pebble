use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;
use std::thread;

const USAGE: &str = "usage: pebble [FILE.pb | -e CODE]   (no arguments starts the REPL)";

fn main() -> ExitCode {
    // Deeply recursive Pebble programs recurse in Rust too, so give the
    // interpreter a roomy stack (the call-depth limit keeps it bounded).
    thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(real_main)
        .expect("spawn interpreter thread")
        .join()
        .unwrap_or(ExitCode::FAILURE)
}

fn real_main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let source = match args.as_slice() {
        [] => return repl(),
        [flag, code] if flag == "-e" => code.clone(),
        [flag] if flag == "-h" || flag == "--help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        [path] => match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("pebble: can't read {path}: {e}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match pebble::run(&source, Box::new(io::BufWriter::new(io::stdout()))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprint!("{}", e.render(&source));
            ExitCode::FAILURE
        }
    }
}

fn repl() -> ExitCode {
    let interactive = io::stdin().is_terminal();
    if interactive {
        println!(
            "pebble {} | type an expression, or Ctrl-D to quit",
            env!("CARGO_PKG_VERSION")
        );
    }
    let mut interp = pebble::interpreter(Box::new(io::stdout()));
    let mut buffer = String::new();
    let mut lines = io::stdin().lock().lines();
    loop {
        if interactive {
            print!("{}", if buffer.is_empty() { "> " } else { ". " });
            io::stdout().flush().ok();
        }
        let Some(Ok(line)) = lines.next() else { break };
        buffer.push_str(&line);
        buffer.push('\n');
        // Keep reading while brackets are open, so multi-line functions work.
        let depth: i32 = buffer
            .chars()
            .map(|c| match c {
                '{' | '(' | '[' => 1,
                '}' | ')' | ']' => -1,
                _ => 0,
            })
            .sum();
        if depth > 0 {
            continue;
        }
        match pebble::parse(&buffer).and_then(|program| interp.run(&program)) {
            Ok(Some(value)) if !matches!(value, pebble::Value::Nil) => println!("{}", value.repr()),
            Ok(_) => {}
            Err(e) => eprint!("{}", e.render(&buffer)),
        }
        buffer.clear();
    }
    if interactive {
        println!();
    }
    ExitCode::SUCCESS
}
