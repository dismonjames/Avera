mod common;
use common::assert_compile_fails;

// ===== R136: Declaration vs assignment =====

#[test]
fn cf_dup_decl_same_scope() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :x = 2
    return 0
finish
"#,
        "already declared",
    );
}

#[test]
fn cf_dup_decl_different_types() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :x = "hello"
    return 0
finish
"#,
        "already declared",
    );
}

#[test]
fn cf_dup_decl_in_if() {
    // Inner-scope shadowing is LEGAL, not a fail. This is tested in runtime_pass.
    // Here we test re-decl in the SAME scope as the if, not inside the if body.
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :x = 2
    if true will
        print(x)
    finish
    return 0
finish
"#,
        "already declared",
    );
}

#[test]
fn cf_dup_decl_in_while() {
    // Re-decl in same scope (the while is separate from the decl).
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :x = 2
    while x < 3 will
        x += 1
    finish
    return 0
finish
"#,
        "already declared",
    );
}

#[test]
fn cf_dup_decl_in_for() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    for i in 0..10 will
        :x = 1
        :x = 2
    finish
    return 0
finish
"#,
        "already declared",
    );
}

#[test]
fn cf_dup_decl_three_times() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :x = 2
    :x = 3
    return 0
finish
"#,
        "already declared",
    );
}

// ===== Ownership: use-after-move =====

#[test]
fn cf_use_after_move() {
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
fn cf_use_after_move_in_print() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    print("x is", x)
    return 0
finish
"#,
        "use after move",
    );
}

#[test]
fn cf_use_after_move_in_expr() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    :z = x + 1
    return 0
finish
"#,
        "use after move",
    );
}

#[test]
fn cf_double_move() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    :z = ^x
    return 0
finish
"#,
        "use after move",
    );
}

#[test]
fn cf_move_in_condition() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    if x > 0 will
        print("yes")
    finish
    return 0
finish
"#,
        "use after move",
    );
}

// ===== Ownership: use-after-drop =====

#[test]
fn cf_use_after_drop() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    x!
    print(x)
    return 0
finish
"#,
        "use after drop",
    );
}

#[test]
fn cf_use_after_drop_in_expr() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    x!
    :y = x + 1
    return 0
finish
"#,
        "use after drop",
    );
}

#[test]
fn cf_drop_then_print() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    x!
    print("x:", x)
    return 0
finish
"#,
        "use after drop",
    );
}

// ===== Ownership: double-drop =====

#[test]
fn cf_double_drop() {
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
fn cf_triple_drop() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    x!
    x!
    x!
    return 0
finish
"#,
        "double drop",
    );
}

// ===== Ownership: drop-after-move =====

#[test]
fn cf_drop_after_move() {
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

// Uninitialized locals cannot currently be written in stage-0 source syntax.
// The ownership invariant itself is covered directly at MIR level in mir_safety.rs.

// ===== Ownership: move then move back (reassign revives) =====

// This should SUCCEED — reassigning after a move reinitializes.
// (Tested in runtime_pass.rs instead.)

// ===== Move into a binding, then use the binding =====

// This should SUCCEED — the target owns the value now.
// (Tested in runtime_pass.rs instead.)

// ===== Multiple moves from the same source (after reassign) =====

#[test]
fn cf_move_then_use_then_move_without_reassign() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    :z = ^x
    return 0
finish
"#,
        "use after move",
    );
}

// ===== Drop then move (dropped value can't be moved) =====

#[test]
fn cf_move_after_drop() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    x!
    :y = ^x
    return 0
finish
"#,
        "use after drop",
    );
}

// ===== Scope: inner shadow then outer use (should work, not a fail) =====
// (Tested in runtime_pass.rs)

// ===== Multiple variables, selective drop =====

#[test]
fn cf_drop_one_use_other_after() {
    // Dropping x should not affect y — but using x after drop is an error.
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    :y = 99
    x!
    print(x)
    return 0
finish
"#,
        "use after drop",
    );
}

// ===== Nested scopes and ownership =====

#[test]
fn cf_use_after_move_across_cfg_join() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 42
    if true will
        :y = ^x
    finish
    print(x)
    return 0
finish
"#,
        "use after move",
    );
}

// ===== Magnets: immutable magnet write should fail (if enforced) =====
// Note: v0.1 may not enforce mutability on magnets at compile time.
// This test is a placeholder for when mutability checking is added.

// ===== Large program with multiple errors =====

#[test]
fn cf_multiple_errors() {
    // The compiler should report at least one error for this program.
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :x = 2
    return 0
finish
"#,
        "already declared",
    );
}
