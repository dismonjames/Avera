use avera_compiler::cli::{parse_args, Command, EmitKind};

fn args(values: &[&str]) -> Vec<String> {
    std::iter::once("avera")
        .chain(values.iter().copied())
        .map(str::to_string)
        .collect()
}

#[test]
fn commands_that_need_inputs_reject_empty_invocations() {
    for command in ["check", "build", "test", "fmt"] {
        let err = parse_args(&args(&[command])).expect_err("empty command should fail");
        assert!(err.contains("needs at least one input"), "{command}: {err}");
    }
}

#[test]
fn invalid_optimization_level_is_not_silently_o0() {
    let err = parse_args(&args(&["build", "--opt", "banana", "main.av"]))
        .expect_err("invalid optimization level should fail");
    assert!(err.contains("invalid optimization level"), "{err}");

    let err = parse_args(&args(&["build", "--opt", "3", "main.av"]))
        .expect_err("out of range optimization level should fail");
    assert!(err.contains("expected 0, 1, or 2"), "{err}");
}

#[test]
fn missing_flag_values_are_errors() {
    assert!(parse_args(&args(&["build", "main.av", "--opt"])).is_err());
    assert!(parse_args(&args(&["build", "main.av", "--emit"])).is_err());
}

#[test]
fn fake_emit_targets_are_rejected_until_implemented() {
    for target in ["typed-ast", "clif"] {
        let err = parse_args(&args(&["build", "--emit", target, "main.av"]))
            .expect_err("unsupported emit target should fail");
        assert!(err.contains("not implemented"), "{target}: {err}");
    }

    let parsed = parse_args(&args(&["build", "--emit", "mir", "main.av"]))
        .expect("implemented emit target should parse");
    match parsed {
        Command::Build { emit, .. } => assert_eq!(emit, Some(EmitKind::Mir)),
        _ => panic!("expected build command"),
    }
}

#[test]
fn check_and_fmt_reject_unknown_flags() {
    assert!(parse_args(&args(&["check", "--mystery", "main.av"])).is_err());
    assert!(parse_args(&args(&["fmt", "--mystery", "main.av"])).is_err());
}

#[test]
fn run_requires_separator_before_flag_like_program_args() {
    let err = parse_args(&args(&["run", "--wat"])).expect_err("unknown run flag should fail");
    assert!(err.contains("use `--`"), "{err}");

    let parsed = parse_args(&args(&["run", "main.av", "--", "--wat"]))
        .expect("program args after separator should parse");
    match parsed {
        Command::Run { args, .. } => assert_eq!(args, vec!["--wat"]),
        _ => panic!("expected run command"),
    }
}
