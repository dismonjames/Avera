#![allow(dead_code)]

use std::process::Command;

fn build_source(src: &str, prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = format!("{}_{}", prefix, id);

    let dir = std::env::temp_dir().join("avera_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join(format!("{}.av", name));
    std::fs::write(&src_path, src).unwrap();

    // The build pipeline outputs to build/debug/ relative to CWD.
    let exe = std::path::PathBuf::from(format!("build/debug/{}", name));

    // Build using the CLI.
    let build = Command::new(env!("CARGO_BIN_EXE_avera"))
        .arg("build")
        .arg(&src_path)
        .output()
        .expect("failed to run avera build");
    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr);
        // Clean up the source even on build failure.
        let _ = std::fs::remove_file(&src_path);
        panic!("build failed:\n{}", stderr);
    }

    // Clean up the source file (we only need the binary).
    let _ = std::fs::remove_file(&src_path);
    exe
}

fn cleanup_binary(exe: &std::path::Path) {
    let _ = std::fs::remove_file(exe);
    let mut obj = exe.to_path_buf();
    obj.set_extension("o");
    let _ = std::fs::remove_file(&obj);
}

fn build_and_run(src: &str) -> String {
    build_and_run_full(src).0
}

fn build_and_run_full(src: &str) -> (String, String) {
    let exe = build_source(src, "test_input");
    // Run the binary.
    let run = Command::new(&exe).output().expect("failed to run binary");
    cleanup_binary(&exe);
    (
        String::from_utf8_lossy(&run.stdout).to_string(),
        String::from_utf8_lossy(&run.stderr).to_string(),
    )
}

fn build_and_run_with_stdin(src: &str, stdin: &str) -> String {
    let exe = build_source(src, "stdin_input");

    use std::io::Write;
    let run = Command::new(&exe)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn binary");
    {
        let mut child_stdin = run.stdin.as_ref().expect("failed to open stdin");
        child_stdin
            .write_all(stdin.as_bytes())
            .expect("failed to write stdin");
    }
    let output = run.wait_with_output().expect("failed to wait on binary");
    cleanup_binary(&exe);
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_compile_fails(src: &str, needle: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static FAIL_COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = FAIL_COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = format!("fail_input_{}", id);

    let dir = std::env::temp_dir().join("avera_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join(format!("{}.av", name));
    std::fs::write(&src_path, src).unwrap();

    let build = Command::new(env!("CARGO_BIN_EXE_avera"))
        .arg("build")
        .arg(&src_path)
        .output()
        .expect("failed to run avera build");
    // Clean up the source file (build failed, so no binary to remove).
    let _ = std::fs::remove_file(&src_path);
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&build.stderr),
        String::from_utf8_lossy(&build.stdout)
    );
    assert!(
        !build.status.success(),
        "expected compilation to FAIL, but it succeeded. Output:\n{}",
        combined
    );
    assert!(
        combined.contains(needle),
        "expected diagnostic containing `{}` but got:\n{}",
        needle,
        combined
    );
    combined
}

#[test]
fn test_hello() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    print("Hello, Avera!")
    return 0
finish
"#,
    );
    assert_eq!(out, "Hello, Avera!\n");
}

#[test]
fn test_arithmetic() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :a = 10
    :b = 20
    :c = a + b
    print("sum:", c)
    return 0
finish
"#,
    );
    assert_eq!(out, "sum: 30\n");
}

#[test]
fn test_while_loop() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :sum = 0
    :i = 0
    while i < 10 will
        sum = sum + i
        i = i + 1
    finish
    print("sum:", sum)
    return 0
finish
"#,
    );
    assert_eq!(out, "sum: 45\n");
}

#[test]
fn test_for_loop() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :sum = 0
    for i in 0..10 will
        sum = sum + i
    finish
    print("sum:", sum)
    return 0
finish
"#,
    );
    assert_eq!(out, "sum: 45\n");
}

#[test]
fn test_if_else_chain() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :score = 75
    if score > 80 will
        print("A")
    else if score > 60 will
        print("B")
    else
        print("C")
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "B\n");
}

#[test]
fn test_nested_control() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    for i in 0..3 will
        if i == 0 will
            print("zero")
        else if i == 1 will
            print("one")
        else
            print("two")
        finish
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "zero\none\ntwo\n");
}

#[test]
fn test_function_calls() {
    let out = build_and_run(
        r#"
#std.io

action square(x: I32): I32 will
    return x * x
finish

action main(): I32 will
    :r = square(7)
    print("square(7)=", r)
    return 0
finish
"#,
    );
    assert_eq!(out, "square(7)= 49\n");
}

#[test]
fn test_compound_assignment() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 10
    x += 5
    x -= 3
    print("x:", x)
    return 0
finish
"#,
    );
    assert_eq!(out, "x: 12\n");
}

#[test]
fn test_match_int() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :n = 2
    match n will
        1 => print("one")
        2 => print("two")
        _ => print("other")
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "two\n");
}

#[test]
fn test_magnet_address() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :value = 42
    ~m = value
    print("Address:", ~m.address)
    return 0
finish
"#,
    );
    // The output should be "Address: <non-zero number>\n"
    let trimmed = out.trim();
    assert!(trimmed.starts_with("Address: "), "got: {}", trimmed);
    let addr_str = trimmed.strip_prefix("Address: ").unwrap();
    let addr: i64 = addr_str.parse().unwrap_or(0);
    assert!(addr != 0, "magnet address should be non-zero, got {}", addr);
}

#[test]
fn test_fibonacci() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :a = 0
    :b = 1
    :i = 0
    while i < 10 will
        :t = a + b
        a = b
        b = t
        i = i + 1
    finish
    print("fib(10) =", b)
    return 0
finish
"#,
    );
    assert_eq!(out, "fib(10) = 89\n");
}

#[test]
fn test_assignment_in_loop() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 1
    :i = 0
    while i < 3 will
        x = x + 10
        i = i + 1
    finish
    print("x:", x)
    return 0
finish
"#,
    );
    assert_eq!(out, "x: 31\n");
}

#[test]
fn test_negative_numbers() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = -5
    :y = 3 - 10
    print("x:", x, "y:", y)
    return 0
finish
"#,
    );
    assert_eq!(out, "x: -5 y: -7\n");
}

#[test]
fn test_float_arithmetic() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :pi = 3.14
    :r = 2.0
    :area = pi * r * r
    print("area:", area)
    return 0
finish
"#,
    );
    assert!(
        out.contains("12.56"),
        "expected 12.56 in output, got: {}",
        out
    );
}

#[test]
fn test_break_continue() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :sum = 0
    for i in 0..10 will
        if i == 5 will
            break
        finish
        sum = sum + i
    finish
    print("sum:", sum)
    return 0
finish
"#,
    );
    assert_eq!(out, "sum: 10\n");
}

#[test]
fn test_nested_loops() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :count = 0
    for i in 0..3 will
        for j in 0..3 will
            count = count + 1
        finish
    finish
    print("count:", count)
    return 0
finish
"#,
    );
    assert_eq!(out, "count: 9\n");
}

#[test]
fn test_boolean_logic() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :t = true
    :f = false
    if t && !f will
        print("yes")
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "yes\n");
}

#[test]
fn test_array_index_and_len() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :arr = [10, 20, 30, 40, 50]
    print("len:", arr.len())
    for i in 0..5 will
        print(arr[i])
    finish
    return 0
finish
"#,
    );
    assert!(out.contains("len: 5"), "got: {}", out);
    for v in ["10", "20", "30", "40", "50"] {
        assert!(out.contains(v), "expected {} in output, got: {}", v, out);
    }
}

#[test]
fn test_array_push_pop() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :arr = [1, 2, 3]
    arr.push(4)
    print("len:", arr.len())
    :p = arr.pop()
    print("popped:", p)
    print("len after pop:", arr.len())
    return 0
finish
"#,
    );
    assert!(out.contains("len: 4"), "got: {}", out);
    assert!(out.contains("popped: 4"), "got: {}", out);
    assert!(out.contains("len after pop: 3"), "got: {}", out);
}

#[test]
fn test_text_store_and_print() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :greeting = "Hello"
    print(greeting)
    print("len:", greeting.len())
    return 0
finish
"#,
    );
    assert!(out.contains("Hello"), "got: {}", out);
    assert!(out.contains("len: 5"), "got: {}", out);
}

#[test]
fn test_text_concat() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :a = "foo"
    :b = "bar"
    :c = a + b
    print(c)
    print("len:", c.len())
    return 0
finish
"#,
    );
    assert!(out.contains("foobar"), "got: {}", out);
    assert!(out.contains("len: 6"), "got: {}", out);
}

#[test]
fn test_text_equality() {
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :a = "foo"
    if a == "foo" will
        print("match")
    finish
    if a != "bar" will
        print("not bar")
    finish
    return 0
finish
"#,
    );
    assert!(out.contains("match"), "got: {}", out);
    assert!(out.contains("not bar"), "got: {}", out);
}

#[test]
fn test_array_bounds_check() {
    // An out-of-bounds array access aborts with a diagnostic.
    let (out, _err) = build_and_run_full(
        r#"
#std.io

action main(): I32 will
    :arr = [1, 2, 3]
    :v = arr[10]
    print("unreachable")
    return 0
finish
"#,
    );
    // The program should abort before printing "unreachable".
    assert!(!out.contains("unreachable"), "got: {}", out);
    assert!(
        out.contains("array index out of bounds"),
        "expected bounds diagnostic, got: {}",
        out
    );
}

#[test]
fn test_magnet_offset() {
    // ~m.offset reports the byte offset within the pointed-to region.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 42
    ~m = x
    print("offset:", ~m.offset)
    return 0
finish
"#,
    );
    assert_eq!(out, "offset: 0\n");
}

#[test]
fn test_magnet_deref() {
    // ~m.value loads the value the magnet points to.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 42
    ~m = x
    print("value:", ~m.value)
    return 0
finish
"#,
    );
    assert_eq!(out, "value: 42\n");
}

#[test]
fn test_magnet_retarget() {
    // `m -> other` re-points the magnet at a new value.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 42
    ~m = x
    print("before:", ~m.value)
    :y = 99
    m -> y
    print("after:", ~m.value)
    return 0
finish
"#,
    );
    assert!(out.contains("before: 42"), "got: {}", out);
    assert!(out.contains("after: 99"), "got: {}", out);
}

#[test]
fn test_explicit_drop() {
    // `place!` explicit drop: after dropping, the value is logically dead.
    // The ownership checker now flags use-after-drop as a compile error.
    assert_compile_fails(
        r#"
#std.io

action main(): I32 will
    :x = 42
    x!
    print("after drop:", x)
    return 0
finish
"#,
        "use after drop",
    );
}

#[test]
fn test_explicit_drop_runtime_resets() {
    // `place!` drops the value (stores 0) but does NOT use it afterward — so
    // no use-after-drop error is raised. This confirms the drop happens and
    // the checker only fires when a dropped value is actually used.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 42
    x!
    print("dropped")
    return 0
finish
"#,
    );
    assert_eq!(out, "dropped\n");
}

#[test]
fn test_duplicate_declaration_fails() {
    // R136: re-declaring `:x` in the same scope is a compile error.
    assert_compile_fails(
        r#"
#std.io

action main(): I32 will
    :x = 1
    :x = 2
    return 0
finish
"#,
        "already declared in this scope",
    );
}

#[test]
fn test_inner_scope_shadow() {
    // R137: an inner scope may shadow an outer binding.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 1
    if true will
        :x = 2
        print(x)
    finish
    print(x)
    return 0
finish
"#,
    );
    assert_eq!(out, "2\n1\n");
}

#[test]
fn test_assignment_to_existing_binding() {
    // R136 companion: `x = 2` is assignment (no `:`), allowed and mutates.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 1
    x = 2
    print(x)
    return 0
finish
"#,
    );
    assert_eq!(out, "2\n");
}

#[test]
fn r138_magnet_write_through() {
    // R138 REQUIRED REGRESSION — MAGNET WRITE.
    // `~!m = value` makes m a mutable magnet aliasing value's storage.
    // `m = 100` stores 100 THROUGH the magnet into value's storage.
    // `print(value)` must therefore print 100 (not the original 0).
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :value = 0
    ~!m = value
    m = 100
    print(value)
    return 0
finish
"#,
    );
    assert_eq!(out, "100\n");
}

#[test]
fn r138_magnet_write_through_preserves_address() {
    // After `m = 100` stores through the magnet, the magnet must STILL point
    // at value (the address must not be clobbered). `~m.value` reads back
    // through the same address and must see 100.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :value = 0
    ~!m = value
    m = 100
    print("~m.value:", ~m.value)
    print("value:", value)
    return 0
finish
"#,
    );
    assert!(out.contains("~m.value: 100"), "got: {}", out);
    assert!(out.contains("value: 100"), "got: {}", out);
}

#[test]
fn r138_magnet_write_compound() {
    // Compound store-through: `m = m.value + 5` reads through, adds, stores.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :value = 10
    ~!m = value
    m = ~m.value + 5
    print(value)
    return 0
finish
"#,
    );
    assert_eq!(out, "15\n");
}

#[test]
fn r139_magnet_field_write() {
    // R139 REQUIRED REGRESSION — MAGNET FIELD.
    // `~!m = pt` makes m alias pt's storage (which holds the struct pointer).
    // `m.x = 16` loads the struct pointer through m, stores 16 at field x.
    // `print(pt.x)` must therefore print 16.
    let out = build_and_run(
        r#"
#std.io

shape Point will
    x: I32
    y: I32
finish

action main(): I32 will
    :pt = Point(0, 0)
    ~!m = pt
    m.x = 16
    print(pt.x)
    return 0
finish
"#,
    );
    assert_eq!(out, "16\n");
}

#[test]
fn r139_magnet_field_write_preserves_address() {
    // After `m.x = 16`, the magnet still points at pt, and reading the field
    // back through the magnet must see 16.
    let out = build_and_run(
        r#"
#std.io

shape Point will
    x: I32
    y: I32
finish

action main(): I32 will
    :pt = Point(10, 20)
    ~!m = pt
    m.x = 16
    print("~m.x:", ~m.x)
    print("pt.x:", pt.x)
    print("pt.y:", pt.y)
    return 0
finish
"#,
    );
    assert!(out.contains("~m.x: 16"), "got: {}", out);
    assert!(out.contains("pt.x: 16"), "got: {}", out);
    assert!(out.contains("pt.y: 20"), "got: {}", out);
}

#[test]
fn r139_direct_field_write() {
    // Companion: regular `pt.x = v` (no magnet) must also work, so that the
    // magnet field-write path and the direct field-write path agree.
    let out = build_and_run(
        r#"
#std.io

shape Point will
    x: I32
    y: I32
finish

action main(): I32 will
    :pt = Point(0, 0)
    pt.x = 7
    pt.y = 9
    print("x:", pt.x)
    print("y:", pt.y)
    return 0
finish
"#,
    );
    assert!(out.contains("x: 7"), "got: {}", out);
    assert!(out.contains("y: 9"), "got: {}", out);
}

#[test]
fn r139_magnet_field_write_via_deref() {
    // `~m.x` reads the field through the magnet after writing through it.
    let out = build_and_run(
        r#"
#std.io

shape Box will
    v: I32
finish

action main(): I32 will
    :b = Box(0)
    ~!m = b
    m.v = 42
    print(~m.v)
    return 0
finish
"#,
    );
    assert_eq!(out, "42\n");
}

#[test]
fn r146_choice_deterministic_discriminant() {
    // R146 REQUIRED REGRESSION — CHOICE REAL DISCRIMINANTS.
    // A declared choice type assigns discriminants by declaration order, NOT
    // a hash. Construct each variant and match it; each arm must fire.
    let out = build_and_run(
        r#"
#std.io

choice Light will
    red
    yellow
    green
finish

action main(): I32 will
    :a = .red
    :b = .yellow
    :c = .green
    match a will
        .red => print("red")
        .yellow => print("yellow")
        .green => print("green")
    finish
    match b will
        .red => print("red")
        .yellow => print("yellow")
        .green => print("green")
    finish
    match c will
        .red => print("red")
        .yellow => print("yellow")
        .green => print("green")
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "red\nyellow\ngreen\n");
}

#[test]
fn r146_choice_payload_extraction() {
    // R146 REQUIRED REGRESSION — CHOICE REAL PAYLOAD EXTRACTION.
    // The payload stored at construction must be extracted correctly in the
    // matching arm, even with multiple payload-carrying variants.
    let out = build_and_run(
        r#"
#std.io

choice Result will
    ok(value: I32)
    err(value: I32)
finish

action main(): I32 will
    :a = .ok(42)
    :b = .err(7)
    match a will
        .ok(v) => print("ok:", v)
        .err(v) => print("err:", v)
    finish
    match b will
        .ok(v) => print("ok:", v)
        .err(v) => print("err:", v)
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "ok: 42\nerr: 7\n");
}

#[test]
fn r146_choice_distinct_per_type() {
    // Discriminants are per-choice-type (declaration index), so two choice
    // types can each have a variant with the same NAME without colliding.
    let out = build_and_run(
        r#"
#std.io

choice A will
    x
    y
finish

choice B will
    y
    x
finish

action main(): I32 will
    :a = .y
    :b = .y
    match a will
        .x => print("A.x")
        .y => print("A.y")
    finish
    match b will
        .y => print("B.y")
        .x => print("B.x")
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "A.y\nB.y\n");
}

#[test]
fn r146_choice_maybe_canonical() {
    // The canonical Maybe (none/some) keeps conventional tags even without a
    // declaration, so floating .some/.none still match correctly.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :m = .some(42)
    match m will
        .some(v) => print("some:", v)
        .none => print("none")
    finish
    :n = .none()
    match n will
        .some(v) => print("some:", v)
        .none => print("none")
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "some: 42\nnone\n");
}

#[test]
fn r146_choice_outcome_canonical() {
    // The canonical Outcome (ok/err) keeps conventional tags.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :a = .ok(1)
    :b = .err(2)
    match a will
        .ok(v) => print("ok:", v)
        .err(v) => print("err:", v)
    finish
    match b will
        .ok(v) => print("ok:", v)
        .err(v) => print("err:", v)
    finish
    return 0
finish
"#,
    );
    assert_eq!(out, "ok: 1\nerr: 2\n");
}

#[test]
fn r_input_returns_text() {
    // input() is typed Text: it reads a whole line, and print() must print the
    // TEXT (not the pointer integer).
    let out = build_and_run_with_stdin(
        r#"
#std.io

action main(): I32 will
    print("enter:")
    :s = input()
    print("you typed:")
    print(s)
    return 0
finish
"#,
        "hello world\n",
    );
    assert_eq!(out, "enter:\nyou typed:\nhello world\n");
}

#[test]
fn r_input_text_concat() {
    // A Text input can be concatenated with string literals — proving the
    // value is a real Text, not a bare integer.
    let out = build_and_run_with_stdin(
        r#"
#std.io

action main(): I32 will
    :name = input()
    :greeting = "Hello, " + name + "!"
    print(greeting)
    return 0
finish
"#,
        "Alice\n",
    );
    assert_eq!(out, "Hello, Alice!\n");
}

#[test]
fn r_input_text_len() {
    // A Text input supports Text operations like .len.
    let out = build_and_run_with_stdin(
        r#"
#std.io

action main(): I32 will
    :s = input()
    print("len:", s.len())
    return 0
finish
"#,
        "abcde\n",
    );
    assert_eq!(out, "len: 5\n");
}

#[test]
fn r_move_use_after_move_fails() {
    // `^x` moves x; using x afterward is a compile error (use-after-move).
    assert_compile_fails(
        r#"
#std.io

action main(): I32 will
    :x = 42
    :y = ^x
    print(x)
    return 0
finish
"#,
        "use after move",
    );
}

#[test]
fn r_move_reassign_after_move_ok() {
    // After moving x, reassigning x reinitializes it, so subsequent use is OK.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 42
    :y = ^x
    x = 99
    print(x)
    return 0
finish
"#,
    );
    assert_eq!(out, "99\n");
}

#[test]
fn r_double_drop_fails() {
    // Dropping an already-dropped value is a compile error (double drop).
    assert_compile_fails(
        r#"
#std.io

action main(): I32 will
    :x = 42
    x!
    x!
    return 0
finish
"#,
        "double drop",
    );
}

#[test]
fn r_drop_after_move_fails() {
    // Dropping a moved value is a compile error (the value is already gone).
    assert_compile_fails(
        r#"
#std.io

action main(): I32 will
    :x = 42
    :y = ^x
    x!
    return 0
finish
"#,
        "moved",
    );
}

#[test]
fn r_move_then_drop_target_ok() {
    // Moving INTO a new binding and then dropping the target is fine; the
    // target owns the value now and can be dropped.
    let out = build_and_run(
        r#"
#std.io

action main(): I32 will
    :x = 42
    :y = ^x
    y!
    print("done")
    return 0
finish
"#,
    );
    assert_eq!(out, "done\n");
}

#[test]
fn r_use_uninit_fails() {
    // Using an uninitialized value is a compile error.
    assert_compile_fails(
        r#"
#std.io

action main(): I32 will
    :x: I32
    print(x)
    return 0
finish
"#,
        "",
    );
}
