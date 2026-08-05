#![allow(dead_code)]

mod common;
use common::{assert_run, assert_run_contains, build_and_run};

// ===== Arithmetic: integers =====

#[test]
fn rp_add() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(3 + 4)
    return 0
finish
"#,
        "7\n",
    );
}

#[test]
fn rp_sub() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(10 - 3)
    return 0
finish
"#,
        "7\n",
    );
}

#[test]
fn rp_sub_negative() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(3 - 10)
    return 0
finish
"#,
        "-7\n",
    );
}

#[test]
fn rp_mul() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(6 * 7)
    return 0
finish
"#,
        "42\n",
    );
}

#[test]
fn rp_div() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(100 / 4)
    return 0
finish
"#,
        "25\n",
    );
}

#[test]
fn rp_mod() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(17 % 5)
    return 0
finish
"#,
        "2\n",
    );
}

#[test]
fn rp_neg_literal() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(-5)
    return 0
finish
"#,
        "-5\n",
    );
}

#[test]
fn rp_neg_expr() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 10
    print(-x)
    return 0
finish
"#,
        "-10\n",
    );
}

#[test]
fn rp_arith_precedence() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(2 + 3 * 4)
    return 0
finish
"#,
        "14\n",
    );
}

#[test]
fn rp_arith_parens() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print((2 + 3) * 4)
    return 0
finish
"#,
        "20\n",
    );
}

#[test]
fn rp_arith_complex() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(1 + 2 * 3 - 4 / 2)
    return 0
finish
"#,
        "5\n",
    );
}

// ===== Compound assignment =====

#[test]
fn rp_add_assign() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 10
    x += 5
    print(x)
    return 0
finish
"#,
        "15\n",
    );
}

#[test]
fn rp_sub_assign() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 10
    x -= 3
    print(x)
    return 0
finish
"#,
        "7\n",
    );
}

#[test]
fn rp_add_assign_loop() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :sum = 0
    :i = 0
    while i < 5 will
        sum += i
        i += 1
    finish
    print(sum)
    return 0
finish
"#,
        "10\n",
    );
}

// ===== Comparison =====

#[test]
fn rp_eq_true() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 3 == 3 will print("eq") finish
    return 0
finish
"#,
        "eq\n",
    );
}

#[test]
fn rp_eq_false() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 3 == 4 will
        print("eq")
    else
        print("ne")
    finish
    return 0
finish
"#,
        "ne\n",
    );
}

#[test]
fn rp_ne() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 3 != 4 will print("ne") finish
    return 0
finish
"#,
        "ne\n",
    );
}

#[test]
fn rp_lt() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 3 < 5 will print("lt") finish
    return 0
finish
"#,
        "lt\n",
    );
}

#[test]
fn rp_le_eq() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 5 <= 5 will print("le") finish
    return 0
finish
"#,
        "le\n",
    );
}

#[test]
fn rp_gt() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 5 > 3 will print("gt") finish
    return 0
finish
"#,
        "gt\n",
    );
}

#[test]
fn rp_ge() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if 5 >= 5 will print("ge") finish
    return 0
finish
"#,
        "ge\n",
    );
}

// ===== Boolean logic =====

#[test]
fn rp_and_true() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if true && 1 == 1 will print("and") finish
    return 0
finish
"#,
        "and\n",
    );
}

#[test]
fn rp_and_false() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if true && false will
        print("yes")
    else
        print("no")
    finish
    return 0
finish
"#,
        "no\n",
    );
}

#[test]
fn rp_or_true() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if false || true will print("or") finish
    return 0
finish
"#,
        "or\n",
    );
}

#[test]
fn rp_not() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if !false will print("not") finish
    return 0
finish
"#,
        "not\n",
    );
}

// ===== Control flow: if/else =====

#[test]
fn rp_if_true() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if true will print("yes") finish
    return 0
finish
"#,
        "yes\n",
    );
}

#[test]
fn rp_if_false() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if false will
        print("yes")
    else
        print("no")
    finish
    return 0
finish
"#,
        "no\n",
    );
}

#[test]
fn rp_else_if() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 5
    if x > 10 will
        print("big")
    else if x > 3 will
        print("medium")
    else
        print("small")
    finish
    return 0
finish
"#,
        "medium\n",
    );
}

#[test]
fn rp_nested_if() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 5
    if x > 0 will
        if x > 3 will
            print("pos-big")
        else
            print("pos-small")
        finish
    finish
    return 0
finish
"#,
        "pos-big\n",
    );
}

// ===== Control flow: while =====

#[test]
fn rp_while_basic() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :i = 0
    while i < 3 will
        print("i:", i)
        i += 1
    finish
    return 0
finish
"#,
        "i: 0\ni: 1\ni: 2\n",
    );
}

#[test]
fn rp_while_false_immediate() {
    assert_run(
        r#"#std.io
action main(): I32 will
    while false will
        print("never")
    finish
    print("done")
    return 0
finish
"#,
        "done\n",
    );
}

#[test]
fn rp_while_countdown() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :i = 3
    while i > 0 will
        print(i)
        i -= 1
    finish
    print("liftoff")
    return 0
finish
"#,
        "3\n2\n1\nliftoff\n",
    );
}

// ===== Control flow: for =====

#[test]
fn rp_for_basic() {
    assert_run(
        r#"#std.io
action main(): I32 will
    for i in 0..3 will
        print(i)
    finish
    return 0
finish
"#,
        "0\n1\n2\n",
    );
}

#[test]
fn rp_for_sum() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :sum = 0
    for i in 0..10 will
        sum += i
    finish
    print(sum)
    return 0
finish
"#,
        "45\n",
    );
}

#[test]
fn rp_for_range_2_5() {
    assert_run(
        r#"#std.io
action main(): I32 will
    for i in 2..5 will
        print(i)
    finish
    return 0
finish
"#,
        "2\n3\n4\n",
    );
}

// ===== Control flow: match on integers =====

#[test]
fn rp_match_int() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 2
    match x will
        1 => print("one")
        2 => print("two")
        3 => print("three")
        _ => print("other")
    finish
    return 0
finish
"#,
        "two\n",
    );
}

#[test]
fn rp_match_int_default() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 99
    match x will
        1 => print("one")
        2 => print("two")
        _ => print("other")
    finish
    return 0
finish
"#,
        "other\n",
    );
}

// ===== Functions =====

#[test]
fn rp_fn_call() {
    assert_run(
        r#"#std.io
action square(x: I32): I32 will
    return x * x
finish
action main(): I32 will
    print(square(5))
    return 0
finish
"#,
        "25\n",
    );
}

#[test]
fn rp_fn_two_params() {
    assert_run(
        r#"#std.io
action add(a: I32, b: I32): I32 will
    return a + b
finish
action main(): I32 will
    print(add(3, 4))
    return 0
finish
"#,
        "7\n",
    );
}

#[test]
fn rp_fn_nested_call() {
    assert_run(
        r#"#std.io
action sq(x: I32): I32 will
    return x * x
finish
action sum_sq(a: I32, b: I32): I32 will
    return sq(a) + sq(b)
finish
action main(): I32 will
    print(sum_sq(3, 4))
    return 0
finish
"#,
        "25\n",
    );
}

#[test]
fn rp_fn_recursion() {
    assert_run(
        r#"#std.io
action fact(n: I32): I32 will
    if n <= 1 will
        return 1
    finish
    return n * fact(n - 1)
finish
action main(): I32 will
    print(fact(5))
    return 0
finish
"#,
        "120\n",
    );
}

#[test]
fn rp_fn_fib() {
    assert_run(
        r#"#std.io
action fib(n: I32): I32 will
    if n < 2 will
        return n
    finish
    return fib(n - 1) + fib(n - 2)
finish
action main(): I32 will
    print(fib(10))
    return 0
finish
"#,
        "55\n",
    );
}

#[test]
fn rp_fn_void() {
    assert_run(
        r#"#std.io
action greet() will
    print("hello")
finish
action main(): I32 will
    greet()
    return 0
finish
"#,
        "hello\n",
    );
}

// ===== Text =====

#[test]
fn rp_text_print() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print("hello world")
    return 0
finish
"#,
        "hello world\n",
    );
}

#[test]
fn rp_text_concat() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :a = "foo"
    :b = "bar"
    print(a + b)
    return 0
finish
"#,
        "foobar\n",
    );
}

#[test]
fn rp_text_concat_literal() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print("Hello, " + "World!")
    return 0
finish
"#,
        "Hello, World!\n",
    );
}

#[test]
fn rp_text_len() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :s = "hello"
    print(s.len())
    return 0
finish
"#,
        "5\n",
    );
}

#[test]
fn rp_text_eq_true() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if "abc" == "abc" will print("eq") finish
    return 0
finish
"#,
        "eq\n",
    );
}

#[test]
fn rp_text_eq_false() {
    assert_run(
        r#"#std.io
action main(): I32 will
    if "abc" == "abd" will
        print("eq")
    else
        print("ne")
    finish
    return 0
finish
"#,
        "ne\n",
    );
}

#[test]
fn rp_text_index() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :s = "abcde"
    print(s[0])
    print(s[4])
    return 0
finish
"#,
        "97\n101\n",
    );
}

#[test]
fn rp_text_three_concat() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :a = "x"
    :b = "y"
    :c = "z"
    print(a + b + c)
    return 0
finish
"#,
        "xyz\n",
    );
}

// ===== Arrays =====

#[test]
fn rp_array_literal() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :arr = [1, 2, 3]
    print(arr[0])
    print(arr[1])
    print(arr[2])
    return 0
finish
"#,
        "1\n2\n3\n",
    );
}

#[test]
fn rp_array_len() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :arr = [10, 20, 30, 40, 50]
    print(arr.len())
    return 0
finish
"#,
        "5\n",
    );
}

#[test]
fn rp_array_iterate() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :arr = [10, 20, 30]
    for i in 0..arr.len() will
        print(arr[i])
    finish
    return 0
finish
"#,
        "10\n20\n30\n",
    );
}

#[test]
fn rp_array_push() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :arr = [1, 2, 3]
    arr.push(4)
    print(arr.len())
    print(arr[3])
    return 0
finish
"#,
        "4\n4\n",
    );
}

#[test]
fn rp_array_empty() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :arr = []
    print(arr.len())
    return 0
finish
"#,
        "0\n",
    );
}

// ===== Shapes =====

#[test]
fn rp_shape_basic() {
    assert_run(
        r#"#std.io
shape Point will
    x: I32
    y: I32
finish
action main(): I32 will
    :pt = Point(3, 4)
    print(pt.x)
    print(pt.y)
    return 0
finish
"#,
        "3\n4\n",
    );
}

#[test]
fn rp_shape_field_write() {
    assert_run(
        r#"#std.io
shape Box will
    v: I32
finish
action main(): I32 will
    :b = Box(0)
    b.v = 42
    print(b.v)
    return 0
finish
"#,
        "42\n",
    );
}

#[test]
fn rp_shape_method() {
    assert_run(
        r#"#std.io
shape Point will
    x: I32
    y: I32
finish
action Point.sum(p: Point): I32 will
    return p.x + p.y
finish
action main(): I32 will
    :pt = Point(10, 20)
    print(pt.sum())
    return 0
finish
"#,
        "30\n",
    );
}

#[test]
fn rp_shape_three_fields() {
    assert_run(
        r#"#std.io
shape Triple will
    a: I32
    b: I32
    c: I32
finish
action main(): I32 will
    :t = Triple(1, 2, 3)
    print(t.a)
    print(t.b)
    print(t.c)
    return 0
finish
"#,
        "1\n2\n3\n",
    );
}

#[test]
fn rp_shape_text_field() {
    assert_run(
        r#"#std.io
shape User will
    name: Text
    age: I32
finish
action main(): I32 will
    :u = User("Alice", 30)
    print(u.name)
    print(u.age)
    return 0
finish
"#,
        "Alice\n30\n",
    );
}

// ===== Choices =====

#[test]
fn rp_choice_some() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :m = .some(42)
    match m will
        .some(v) => print(v)
        .none => print("none")
    finish
    return 0
finish
"#,
        "42\n",
    );
}

#[test]
fn rp_choice_none() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :m = .none()
    match m will
        .some(v) => print(v)
        .none => print("none")
    finish
    return 0
finish
"#,
        "none\n",
    );
}

#[test]
fn rp_choice_ok() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :r = .ok(99)
    match r will
        .ok(v) => print("ok:", v)
        .err(v) => print("err:", v)
    finish
    return 0
finish
"#,
        "ok: 99\n",
    );
}

#[test]
fn rp_choice_err() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :r = .err(7)
    match r will
        .ok(v) => print("ok:", v)
        .err(v) => print("err:", v)
    finish
    return 0
finish
"#,
        "err: 7\n",
    );
}

#[test]
fn rp_choice_declared() {
    assert_run(
        r#"#std.io
choice Color will
    red
    green
    blue
finish
action main(): I32 will
    :c = .green
    match c will
        .red => print("red")
        .green => print("green")
        .blue => print("blue")
    finish
    return 0
finish
"#,
        "green\n",
    );
}

#[test]
fn rp_choice_payload_declared() {
    assert_run(
        r#"#std.io
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
        "ok: 42\nerr: 7\n",
    );
}

// ===== Magnets =====

#[test]
fn rp_magnet_value() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    ~m = x
    print(~m.value)
    return 0
finish
"#,
        "42\n",
    );
}

#[test]
fn rp_magnet_address() {
    let _out = build_and_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    ~m = x
    print(~m.address != 0)
    return 0
finish
"#,
    );
    assert_run_contains(
        r#"#std.io
action main(): I32 will
    :x = 42
    ~m = x
    if ~m.address != 0 will print("has addr") finish
    return 0
finish
"#,
        "has addr",
    );
}

#[test]
fn rp_magnet_write_through() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :value = 0
    ~!m = value
    m = 100
    print(value)
    return 0
finish
"#,
        "100\n",
    );
}

#[test]
fn rp_magnet_field_write() {
    assert_run(
        r#"#std.io
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
        "16\n",
    );
}

#[test]
fn rp_magnet_retarget() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    ~m = x
    :y = 99
    m -> y
    print(~m.value)
    return 0
finish
"#,
        "99\n",
    );
}

#[test]
fn rp_magnet_field_read() {
    assert_run(
        r#"#std.io
shape Box will
    v: I32
finish
action main(): I32 will
    :b = Box(42)
    ~!m = b
    print(~m.v)
    return 0
finish
"#,
        "42\n",
    );
}

// ===== Ownership: move =====

#[test]
fn rp_move_basic() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    print(y)
    return 0
finish
"#,
        "42\n",
    );
}

#[test]
fn rp_move_reassign() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    x = 99
    print(x)
    print(y)
    return 0
finish
"#,
        "99\n42\n",
    );
}

#[test]
fn rp_drop_basic() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    x!
    print("dropped")
    return 0
finish
"#,
        "dropped\n",
    );
}

#[test]
fn rp_move_then_drop_target() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 42
    :y = ^x
    y!
    print("done")
    return 0
finish
"#,
        "done\n",
    );
}

// ===== Scoping =====

#[test]
fn rp_inner_scope_shadow() {
    assert_run(
        r#"#std.io
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
        "2\n1\n",
    );
}

#[test]
fn rp_assignment_not_redecl() {
    assert_run(
        r#"#std.io
action main(): I32 will
    :x = 1
    x = 2
    x = 3
    print(x)
    return 0
finish
"#,
        "3\n",
    );
}

// ===== FizzBuzz =====

#[test]
fn rp_fizzbuzz() {
    assert_run(
        r#"#std.io
action main(): I32 will
    for i in 1..16 will
        if i % 15 == 0 will
            print("FizzBuzz")
        else if i % 3 == 0 will
            print("Fizz")
        else if i % 5 == 0 will
            print("Buzz")
        else
            print(i)
        finish
    finish
    return 0
finish
"#,
        "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz\n",
    );
}

// ===== Primes =====

#[test]
fn rp_primes() {
    assert_run(
        r#"#std.io
action main(): I32 will
    for n in 2..12 will
        :d = 2
        while n % d != 0 will
            d += 1
        finish
        if d == n will
            print(n, "is prime")
        finish
    finish
    return 0
finish
"#,
        "2 is prime\n3 is prime\n5 is prime\n7 is prime\n11 is prime\n",
    );
}

// ===== Multi-arg print =====

#[test]
fn rp_print_multi() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print("a:", 1, "b:", 2)
    return 0
finish
"#,
        "a: 1 b: 2\n",
    );
}

#[test]
fn rp_print_bool() {
    assert_run(
        r#"#std.io
action main(): I32 will
    print(true)
    print(false)
    return 0
finish
"#,
        "true\nfalse\n",
    );
}
