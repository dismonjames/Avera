mod common;

use common::{assert_compile_fails, assert_run};

#[test]
fn range_for_continue_still_advances_iterator() {
    assert_run(
        r#"
#std.io
action main(): I32 will
    :count = 0
    for i in 0..3 will
        count += 1
        if i == 1 && count == 2 will
            continue
        finish
    finish
    print(count)
    return 0
finish
"#,
        "3\n",
    );
}

#[test]
fn break_outside_loop_is_a_compile_error() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    break
    return 0
finish
"#,
        "break` used outside loop",
    );
}

#[test]
fn continue_outside_loop_is_a_compile_error() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    continue
    return 0
finish
"#,
        "continue` used outside loop",
    );
}

#[test]
fn non_range_for_is_rejected_until_it_is_implemented() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :arr = [1, 2, 3]
    for item in arr will
        print(item)
    finish
    return 0
finish
"#,
        "non-range for iterators are not implemented",
    );
}

#[test]
fn eprint_is_rejected_instead_of_silently_using_stdout() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    eprint("err", 42)
    return 0
finish
"#,
        "eprint` is not implemented as stderr output",
    );
}

#[test]
fn retargeting_read_magnet_does_not_upgrade_mutability() {
    assert_compile_fails(
        r#"
#std.io
action main(): I32 will
    :x = 1
    :y = 2
    ~m = x
    m -> y
    m = 3
    return 0
finish
"#,
        "immutable magnet",
    );
}
