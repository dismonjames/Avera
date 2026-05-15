use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match avera_compiler::cli::parse_args(&args) {
        Ok(cmd) => avera_compiler::cli::run(cmd),
        Err(e) => {
            eprintln!("avera: {}", e);
            ExitCode::from(avera_compiler::cli::EXIT_USER)
        }
    }
}
