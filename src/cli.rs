use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone, Debug)]
pub enum Command {
    Check {
        inputs: Vec<PathBuf>,
    },
    Build {
        inputs: Vec<PathBuf>,
        opt: u32,
        emit: Option<EmitKind>,
    },
    Run {
        input: PathBuf,
        args: Vec<String>,
    },
    Test {
        inputs: Vec<PathBuf>,
    },
    Fmt {
        inputs: Vec<PathBuf>,
        check: bool,
    },
    Version,
    Explain {
        code: String,
    },
    Clean,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmitKind {
    Ast,
    Mir,
    Obj,
}

pub const EXIT_OK: u8 = 0;
pub const EXIT_USER: u8 = 1;
pub const EXIT_INTERNAL: u8 = 2;

pub fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut iter = args.iter().skip(1);
    let cmd = match iter.next() {
        None => return Err("no command given; try `avera --help`".into()),
        Some(c) => c.clone(),
    };
    match cmd.as_str() {
        "version" | "--version" | "-V" => Ok(Command::Version),
        "check" => {
            let inputs = collect_paths(&mut iter, "check")?;
            Ok(Command::Check { inputs })
        }
        "build" => {
            let mut opt = 0u32;
            let mut emit = None;
            let mut inputs = Vec::new();
            let rest: Vec<String> = iter.cloned().collect();
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "-O0" => opt = 0,
                    "-O1" => opt = 1,
                    "-O2" => opt = 2,
                    "--opt" => {
                        i += 1;
                        let value = rest.get(i).ok_or("`--opt` needs a value 0, 1, or 2")?;
                        opt = value
                            .parse::<u32>()
                            .map_err(|_| format!("invalid optimization level `{value}`"))?;
                        if opt > 2 {
                            return Err(format!(
                                "invalid optimization level `{opt}`; expected 0, 1, or 2"
                            ));
                        }
                    }
                    "--emit" => {
                        i += 1;
                        let value = rest.get(i).ok_or("`--emit` needs a target")?;
                        emit = Some(parse_emit(value)?);
                    }
                    other if other.starts_with('-') => {
                        return Err(format!("unknown flag `{}`", other));
                    }
                    p => inputs.push(PathBuf::from(p)),
                }
                i += 1;
            }
            require_inputs("build", &inputs)?;
            Ok(Command::Build { inputs, opt, emit })
        }
        "run" => {
            let mut input = None;
            let mut args = Vec::new();
            for a in iter.by_ref() {
                if a == "--" {
                    args.extend(iter.by_ref().cloned());
                    break;
                }
                if input.is_none() {
                    if a.starts_with('-') {
                        return Err(format!(
                            "unknown `run` flag `{a}`; use `--` before program args"
                        ));
                    }
                    input = Some(PathBuf::from(a));
                } else {
                    args.push(a.clone());
                }
            }
            Ok(Command::Run {
                input: input.ok_or("`avera run` needs a source file")?,
                args,
            })
        }
        "test" => Ok(Command::Test {
            inputs: collect_paths(&mut iter, "test")?,
        }),
        "fmt" => {
            let mut check = false;
            let mut inputs = Vec::new();
            for a in iter.by_ref() {
                if a == "--check" {
                    check = true;
                } else if a.starts_with('-') {
                    return Err(format!("unknown `fmt` flag `{a}`"));
                } else {
                    inputs.push(PathBuf::from(a));
                }
            }
            require_inputs("fmt", &inputs)?;
            Ok(Command::Fmt { inputs, check })
        }
        "clean" => Ok(Command::Clean),
        "explain" => {
            let code = iter.next().cloned().ok_or("`avera explain` needs a code")?;
            if iter.next().is_some() {
                return Err("`avera explain` accepts exactly one diagnostic code".into());
            }
            Ok(Command::Explain { code })
        }
        "--help" | "-h" | "help" => {
            print_help();
            std::process::exit(0);
        }
        other => Err(format!("unknown command `{}`; try `avera --help`", other)),
    }
}

fn collect_paths<'a, I: Iterator<Item = &'a String>>(
    iter: &mut I,
    command: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for value in iter {
        if value.starts_with('-') {
            return Err(format!("unknown `{command}` flag `{value}`"));
        }
        paths.push(PathBuf::from(value));
    }
    require_inputs(command, &paths)?;
    Ok(paths)
}

fn require_inputs(command: &str, inputs: &[PathBuf]) -> Result<(), String> {
    if inputs.is_empty() {
        Err(format!("`avera {command}` needs at least one input"))
    } else {
        Ok(())
    }
}

fn parse_emit(s: &str) -> Result<EmitKind, String> {
    Ok(match s {
        "ast" => EmitKind::Ast,
        "mir" => EmitKind::Mir,
        "obj" => EmitKind::Obj,
        "typed-ast" | "clif" => {
            return Err(format!(
                "`--emit {s}` is not implemented in the stage-0 compiler"
            ));
        }
        other => return Err(format!("unknown --emit target `{}`", other)),
    })
}

pub fn print_help() {
    println!(
        "avera — the Avera compiler, version {}",
        crate::AVERA_VERSION
    );
    println!();
    println!("USAGE:");
    println!("    avera <COMMAND> [OPTIONS] [INPUTS...]");
    println!();
    println!("COMMANDS:");
    println!("    check <files...>        Parse, lower, and validate without linking");
    println!("    build <files...>        Compile and link to a native executable");
    println!("    run   <file> [args...]  Build then run the program");
    println!("    test  <dirs...>         Compile and run the test suite");
    println!("    fmt   <files...>        Format source (use --check to verify)");
    println!("    version                Print compiler version");
    println!("    explain <code>         Explain a diagnostic code");
    println!("    clean                  Remove build artifacts");
    println!();
    println!("OPTIONS:");
    println!("    -O0/-O1/-O2 | --opt N  Optimization level");
    println!("    --emit ast|mir|obj      Emit an implemented intermediate form");
    println!("    --check (with fmt)      Exit non-zero if formatting would change");
    println!();
    println!("ENV:");
    println!("    AVERA_DUMP_AST=1   dump the parsed AST");
    println!("    AVERA_DUMP_MIR=1   dump the built MIR");
    println!("    AVERA_DUMP_CLIF=1  dump Cranelift IR");
    println!();
    println!("EXIT CODES:");
    println!("    0  success");
    println!("    1  user/compile error");
    println!("    2  compiler-internal error");
}

pub fn run(cmd: Command) -> ExitCode {
    use crate::driver_pipeline;
    match cmd {
        Command::Version => {
            println!("Avera {} — Avera stage-0 compiler", crate::AVERA_VERSION);
            ExitCode::from(EXIT_OK)
        }
        Command::Explain { code } => match crate::diagnostics_explanations(&code) {
            Some(text) => {
                println!("{}", text);
                ExitCode::from(EXIT_OK)
            }
            None => {
                eprintln!("no explanation available for code `{}`", code);
                ExitCode::from(EXIT_USER)
            }
        },
        Command::Clean => match std::fs::remove_dir_all("build") {
            Ok(()) => {
                println!("cleaned");
                ExitCode::from(EXIT_OK)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("already clean");
                ExitCode::from(EXIT_OK)
            }
            Err(e) => {
                eprintln!("failed to clean build directory: {e}");
                ExitCode::from(EXIT_USER)
            }
        },
        Command::Check { inputs } => match driver_pipeline::check(&inputs) {
            Ok(()) => ExitCode::from(EXIT_OK),
            Err(_) => ExitCode::from(EXIT_USER),
        },
        Command::Build { inputs, opt, emit } => match driver_pipeline::build(&inputs, opt, emit) {
            Ok(()) => ExitCode::from(EXIT_OK),
            Err(_) => ExitCode::from(EXIT_USER),
        },
        Command::Run { input, args } => match driver_pipeline::run(&input, &args) {
            Ok(code) => ExitCode::from(code),
            Err(_) => ExitCode::from(EXIT_USER),
        },
        Command::Test { inputs } => match driver_pipeline::test(&inputs) {
            Ok(()) => ExitCode::from(EXIT_OK),
            Err(_) => ExitCode::from(EXIT_USER),
        },
        Command::Fmt { inputs, check } => match driver_pipeline::fmt(&inputs, check) {
            Ok(()) => ExitCode::from(EXIT_OK),
            Err(_) => ExitCode::from(EXIT_USER),
        },
    }
}
