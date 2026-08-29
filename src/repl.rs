use crate::{evaluator::Evaluator, lexer::Lexer, parser::Parser};
use std::io::{self, BufRead, Write};

const PROMPT: &str = ">> ";

pub fn start() {
    let stdin = io::stdin();
    let mut input_stream = stdin.lock();
    let mut stdout = io::stdout();

    let mut evaluator = Evaluator::new();

    loop {
        // A closed terminal is an ordinary way for a session to end, not a
        // failure to report: `flush().unwrap()` turned it into a panic.
        if write!(stdout, "{PROMPT}").is_err() || stdout.flush().is_err() {
            return;
        }

        // Read raw bytes rather than `read_line`, which fails with
        // `InvalidData` on a line that is not UTF-8 and reached an `.unwrap()`:
        // one pasted Latin-1 character ended the session with a Rust panic and
        // a backtrace note. Consuming the line before validating it also
        // guarantees progress, so a bad line cannot be re-read forever.
        let mut raw = Vec::new();
        match input_stream.read_until(b'\n', &mut raw) {
            Ok(0) => return,
            Ok(_) => {}
            Err(error) => {
                eprintln!("❌ ERROR reading from stdin: {error}");
                return;
            }
        }
        let input = match String::from_utf8(raw) {
            Ok(text) => text,
            Err(_) => {
                // Same uncoded shape its two siblings use for the same
                // condition — `ERROR reading file` and `ERROR reading
                // stdin` — plus what the REPL did about it, since unlike
                // them it continues instead of exiting.
                eprintln!(
                    "❌ ERROR reading stdin: this line did not contain valid UTF-8; the line was skipped."
                );
                continue;
            }
        };

        let source_lines: Vec<String> = input.lines().map(|l| l.to_string()).collect();

        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        // Without this the REPL printed a bare message where the same error in
        // a file showed the offending line and a caret under the column.
        parser.set_source(source_lines.clone());
        let program = parser.parse_program();

        if parser.has_errors() {
            // The same rule `run_source` states: a program with parse errors
            // must not half-run. The REPL used to print the diagnostic and then
            // evaluate anyway, so `out "x"; let y = ;` printed `x` here while
            // the identical line in a file aborted without running anything.
            eprintln!("❌ Aborted: fix the parse errors above before running.");
            continue;
        }

        evaluator.set_source(source_lines);

        // eval_program retorna Option<ObjectRef> — sin clonar datos
        if let Some(obj_ref) = evaluator.eval_program(&program) {
            if writeln!(stdout, "{}", evaluator.display(obj_ref)).is_err() {
                return;
            }
        }
    }
}
