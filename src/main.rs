use anyhow::Context;
use clap::{Parser, Subcommand};
use cap::error::format_error;
use cap::interpreter::Interpreter;
use cap::lexer::Lexer;
use cap::parser::Parser as CapParser;

#[derive(Parser)]
#[command(name = "cap", about = "The cap scripting language")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Script file to run directly (cap <file.fx>)
    file: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Run a cap script file
    Run {
        file: String,
        /// Additional arguments passed to the script as `args`
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Check syntax without executing
    Check { file: String },
    /// Print the AST of a script
    Ast { file: String },
    /// Start interactive REPL
    Repl,
}

fn main() {
    // Spawn on a 64 MB stack so recursive cap scripts don't overflow.
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handler = builder
        .spawn(|| {
            let cli = Cli::parse();
            let result = match cli.command {
                Some(Command::Run { file, args }) => run_file(&file, args),
                Some(Command::Check { file })     => check_file(&file),
                Some(Command::Ast { file })       => ast_file(&file),
                Some(Command::Repl)               => run_repl(),
                None => {
                    if let Some(file) = cli.file {
                        run_file(&file, vec![])
                    } else {
                        run_repl()
                    }
                }
            };
            if let Err(e) = result {
                eprintln!("{e}");
                std::process::exit(1);
            }
        })
        .expect("failed to spawn main thread");
    handler.join().expect("main thread panicked");
}

fn load_source(path: &str) -> anyhow::Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("cannot read file `{path}`"))
}

fn run_file(path: &str, script_args: Vec<String>) -> anyhow::Result<()> {
    let source = load_source(path)?;

    let tokens = Lexer::new(&source)
        .tokenize_all()
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;

    let stmts = CapParser::new(tokens)
        .parse_program()
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;

    let mut interp = Interpreter::new();

    // Expose script args as `args` variable
    use cap::interpreter::value::Value;
    use std::cell::RefCell;
    use std::rc::Rc;
    let args_val = Value::List(Rc::new(RefCell::new(
        script_args.into_iter().map(Value::Str).collect(),
    )));
    interp.set_var("args", args_val);

    interp
        .run_program(&stmts)
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;

    Ok(())
}

fn check_file(path: &str) -> anyhow::Result<()> {
    let source = load_source(path)?;
    let tokens = Lexer::new(&source)
        .tokenize_all()
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;
    CapParser::new(tokens)
        .parse_program()
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;
    println!("OK: {path}");
    Ok(())
}

fn ast_file(path: &str) -> anyhow::Result<()> {
    let source = load_source(path)?;
    let tokens = Lexer::new(&source)
        .tokenize_all()
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;
    let stmts = CapParser::new(tokens)
        .parse_program()
        .map_err(|e| anyhow::anyhow!("{}", format_error(&e, &source, path)))?;
    println!("{stmts:#?}");
    Ok(())
}

fn run_repl() -> anyhow::Result<()> {
    use rustyline::DefaultEditor;
    let mut rl = DefaultEditor::new()?;
    let mut interp = Interpreter::new();
    println!("cap repl — type expressions or assignments, Ctrl-D to exit");

    loop {
        let line = match rl.readline("» ") {
            Ok(l) => l,
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(rustyline::error::ReadlineError::Interrupted) => continue,
            Err(e) => return Err(e.into()),
        };

        if line.trim().is_empty() { continue; }
        let _ = rl.add_history_entry(&line);

        let tokens = match Lexer::new(&line).tokenize_all() {
            Ok(t) => t,
            Err(e) => { eprintln!("{}", format_error(&e, &line, "<repl>")); continue; }
        };
        let stmts = match CapParser::new(tokens).parse_program() {
            Ok(s) => s,
            Err(e) => { eprintln!("{}", format_error(&e, &line, "<repl>")); continue; }
        };
        match interp.run_program(&stmts) {
            Ok(v) if !matches!(v, cap::interpreter::value::Value::Null) => println!("{}", v.repr()),
            Ok(_) => {}
            Err(e) => eprintln!("{}", format_error(&e, &line, "<repl>")),
        }
    }

    Ok(())
}
