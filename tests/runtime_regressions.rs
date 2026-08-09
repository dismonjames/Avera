mod common;

use common::{build_and_run_with_stdin, assert_run};

#[test]
fn array_push_grows_without_invalidating_owner() {
    assert_run(
        r#"
#std.io
action main(): I32 will
    :arr = [7]
    for i in 0..40 will
        arr.push(i)
    finish
    print("len:", arr.len())
    print("last:", arr[40])
    return 0
finish
"#,
        "len: 41\nlast: 39\n",
    );
}

#[test]
fn input_is_not_silently_truncated_at_255_bytes() {
    let input = format!("{}\n", "a".repeat(400));
    let out = build_and_run_with_stdin(
        r#"
#std.io
action main(): I32 will
    :text = input()
    print(text.len())
    return 0
finish
"#,
        &input,
    );
    assert_eq!(out, "400\n");
}
