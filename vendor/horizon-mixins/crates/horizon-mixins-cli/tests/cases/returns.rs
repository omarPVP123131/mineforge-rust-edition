use super::*;

/// A RETURN hook on `fixture_game::run`, whose body is `body`. The helper logs, then adds one.
fn fixture(body: &str) -> Fixture {
    let f = Fixture::new();
    fs::remove_dir_all(&f.mods).unwrap();
    fs::create_dir_all(f.mods.join("alpha")).unwrap();
    fs::write(
        f.root.join("engine/src/lib.rs"),
        format!(
            r#"
pub static LOG: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
pub fn mark(what: &'static str) {{ LOG.lock().unwrap().push(what); }}
pub struct Guard(pub &'static str, pub i32);
impl Guard {{ pub fn new(name: &'static str, n: i32) -> Self {{ Guard(name, n) }} }}
impl Drop for Guard {{ fn drop(&mut self) {{ mark(self.0); }} }}
pub fn size(g: &Guard) -> i32 {{ g.1 }}
pub fn sink(n: i32) -> i32 {{ n }}
pub fn run(early: bool) -> i32 {{ {body} }}
"#
        ),
    )
    .unwrap();
    fs::write(f.root.join("driver/src/lib.rs"), "").unwrap();
    fs::write(
        f.mods.join("alpha/horizon-mixin.toml"),
        r#"
[mod]
id="alpha"
version="1"
[[injection]]
id="return"
target="fixture_game::run"
kind="RETURN"
helper="return.rs"
bindings=[{name="value",ty="i32",value="$value"}]
"#,
    )
    .unwrap();
    fs::write(
        f.mods.join("alpha/return.rs"),
        "fn modify(value:i32)->i32 { crate::mark(\"helper\"); value+1 }",
    )
    .unwrap();
    f
}

#[test]
fn return_hooks_leave_references_to_calls_and_closures_outside_exit_values_alone() {
    // As in Horizon's `picked_stack`: `decode(Some(&Tag::Compound(all)))` in a statement,
    // closures in a condition, and a reference whose temporary a `let` extends to the end of the
    // function. A closure's own `return` leaves the closure and is not the hook's.
    let f = fixture(
        r#"
        let inner = |n: i32| { if n > 0 { return n * 10; } 0 };
        let arg = sink(size(&Guard::new("argument", 2))) + Some(0).map(|n| n + inner(1) - inner(0)).unwrap();
        let extended = &Guard::new("extended", 3);
        if early { return sink(arg); }
        mark("body");
        sink(arg + size(extended))
    "#,
    );
    fs::write(
        f.root.join("driver/src/lib.rs"),
        r#"
#[test]
fn temporaries_keep_their_lifetimes() {
    use fixture_game::{run, LOG};
    // `inner(1)` takes the closure's own `return` (10): the helper runs once per call of `run`.
    assert_eq!(run(false), 16);
    assert_eq!(*LOG.lock().unwrap(), ["argument", "body", "helper", "extended"]);
    LOG.lock().unwrap().clear();
    assert_eq!(run(true), 13);
    assert_eq!(*LOG.lock().unwrap(), ["argument", "helper", "extended"]);
}
"#,
    )
    .unwrap();
    f.run("check");
}

#[test]
fn return_hooks_still_refuse_references_to_calls_and_closures_in_the_values_they_move() {
    for body in [
        "if early { return *&sink(1); } sink(2)",
        "if early { return sink(1); } *&sink(2)",
        "if early { return sink(1); } { let x = &Guard::new(\"tail\", 2); size(x) }",
    ] {
        let f = fixture(body);
        let e = build_plan(&f.root, &f.mods).unwrap_err();
        assert_eq!(e.code, "E_UNSUPPORTED_CONTROL_FLOW", "{body}: {e}");
        if body.contains('&') {
            assert!(e.message.contains("beneath a reference"), "{body}: {e}");
        }
    }
    // What a closure holds is still checked, also in an exit value: `?` and async stay refused.
    for body in [
        "let f = || -> Option<i32> { Some(1)?; None }; sink(f().unwrap_or(2))",
        "let f = async || 1; drop(f); sink(2)",
        "if early { return (|| -> Option<i32> { Some(1)?; None })().unwrap_or(1); } sink(2)",
        "if early { return sink(1); } { let f = async || 1; drop(f); sink(2) }",
    ] {
        let f = fixture(body);
        let e = build_plan(&f.root, &f.mods).unwrap_err();
        assert_eq!(e.code, "E_UNSUPPORTED_CONTROL_FLOW", "{body}: {e}");
    }
    // A hook that rewrites one site still refuses a site that borrows a call's result.
    let f = fixture(r#"let arg = sink(size(&Guard::new("argument", 2))); sink(arg)"#);
    f.append(
        "mods/alpha/horizon-mixin.toml",
        r#"
[[injection]]
id="before"
target="fixture_game::run"
kind="BEFORE_CALL"
selector="sink"
require=2
ordinal=0
helper="before.rs"
"#,
    );
    fs::write(f.mods.join("alpha/before.rs"), "fn before() {}").unwrap();
    let e = build_plan(&f.root, &f.mods).unwrap_err();
    assert_eq!(e.code, "E_UNSUPPORTED_CONTROL_FLOW", "{e}");
    assert!(e.message.contains("beneath a reference"), "{e}");
}

#[test]
fn return_hooks_capture_locals_past_closures_with_their_own_returns_and_lets() {
    // A closure's `return` is not an exit of `run`, and its `let later` is not `later:0`.
    let f = fixture(
        r#"
        let f = |n: i32| { let later = n * 2; if n > 0 { return later; } 0 };
        let later = f(1) + 1;
        if early { return sink(later); }
        sink(later)
    "#,
    );
    fs::write(
        f.mods.join("alpha/horizon-mixin.toml"),
        r#"
[mod]
id="alpha"
version="1"
[[injection]]
id="return"
target="fixture_game::run"
kind="RETURN"
helper="return.rs"
bindings=[{name="value",ty="i32",value="$value"},{name="later",ty="&i32",value="$local_ref:later:0"}]
"#,
    )
    .unwrap();
    fs::write(
        f.mods.join("alpha/return.rs"),
        "fn modify(value:i32, later:&i32)->i32 { crate::mark(\"helper\"); value + *later * 100 }",
    )
    .unwrap();
    fs::write(
        f.root.join("driver/src/lib.rs"),
        r#"
#[test]
fn the_outer_local_is_captured() {
    assert_eq!(fixture_game::run(false), 303);
    assert_eq!(fixture_game::run(true), 303);
}
"#,
    )
    .unwrap();
    f.run("check");
}

#[test]
fn return_hooks_move_closures_in_exit_values_with_the_value() {
    // A closure in a `return` operand and in the tail moves into the helper's argument with the
    // value. The first one's own `return` leaves the closure and is not an exit of `run`.
    let f = fixture(
        r#"
        let base = sink(2);
        if early { return [1, 2].iter().map(|n| { if *n > 1 { return n * 100; } n + base }).sum(); }
        mark("body");
        [3, 4].map(|n| n * base).iter().copied().sum::<i32>() + (|| sink(1))()
    "#,
    );
    fs::write(
        f.root.join("driver/src/lib.rs"),
        r#"
#[test]
fn the_helper_sees_each_exit_value_once() {
    use fixture_game::{run, LOG};
    // 1 + 2, then the closure's own `return` 200: 203, plus the helper's 1.
    assert_eq!(run(true), 204);
    assert_eq!(*LOG.lock().unwrap(), ["helper"]);
    LOG.lock().unwrap().clear();
    // 6 + 8 + 1, plus the helper's 1.
    assert_eq!(run(false), 16);
    assert_eq!(*LOG.lock().unwrap(), ["body", "helper"]);
}
"#,
    )
    .unwrap();
    f.run("check");
}

#[test]
fn site_hooks_leave_borrows_elsewhere_alone_and_wrap_values_beneath_a_borrow() {
    // The borrow of `Guard::new("argument", 2)` is in another statement than the selected
    // `sink(arg)`, and `&Guard::new("extended", 3)` is the selected site of a WRAP_OPERATION:
    // its result is still the temporary the `let` extends to the end of `run`.
    let f = fixture(
        r#"
        let arg = sink(size(&Guard::new("argument", 2)));
        let extended = &Guard::new("extended", 3);
        mark("body");
        sink(arg + size(extended))
    "#,
    );
    f.append(
        "mods/alpha/horizon-mixin.toml",
        r#"
[[injection]]
id="before"
target="fixture_game::run"
kind="BEFORE_CALL"
selector="sink"
require=2
ordinal=1
helper="before.rs"

[[injection]]
id="wrap"
target="fixture_game::run"
kind="WRAP_OPERATION"
selector="Guard::new"
require=2
ordinal=1
helper="wrap.rs"
bindings=[{name="name",ty="&'static str",value="$arg0"},{name="n",ty="i32",value="$arg1"},{name="original",ty="impl FnOnce(&'static str, i32) -> crate::Guard",value="$operation"}]
"#,
    );
    fs::write(
        f.mods.join("alpha/before.rs"),
        "fn before() { crate::mark(\"before\"); }",
    )
    .unwrap();
    fs::write(
        f.mods.join("alpha/wrap.rs"),
        "fn wrap(name: &'static str, n: i32, original: impl FnOnce(&'static str, i32) -> crate::Guard) -> crate::Guard { crate::mark(\"wrap\"); original(name, n + 1) }",
    )
    .unwrap();
    fs::write(
        f.root.join("driver/src/lib.rs"),
        r#"
#[test]
fn temporaries_keep_their_lifetimes() {
    use fixture_game::{run, LOG};
    assert_eq!(run(false), 7);
    assert_eq!(
        *LOG.lock().unwrap(),
        ["argument", "wrap", "body", "before", "helper", "extended"]
    );
}
"#,
    )
    .unwrap();
    f.run("check");
}

#[test]
fn statement_hooks_refuse_a_site_beneath_a_borrow() {
    let f = fixture(r#"let extended = &Guard::new("extended", 3); sink(size(extended))"#);
    f.append(
        "mods/alpha/horizon-mixin.toml",
        r#"
[[injection]]
id="before"
target="fixture_game::run"
kind="BEFORE_CALL"
selector="Guard::new"
require=1
ordinal=0
helper="before.rs"
"#,
    );
    fs::write(f.mods.join("alpha/before.rs"), "fn before() {}").unwrap();
    let e = build_plan(&f.root, &f.mods).unwrap_err();
    assert_eq!(e.code, "E_UNSUPPORTED_CONTROL_FLOW", "{e}");
    assert!(e.message.contains("beneath a reference"), "{e}");
}

#[test]
fn overwrite_drops_a_body_that_borrows_a_call_or_holds_closures() {
    let f = fixture(
        r#"let extended = &Guard::new("extended", 3); let n = Some(1).map(|n| n + 1)?; sink(size(extended) + n)"#,
    );
    fs::write(
        f.root.join("engine/src/lib.rs"),
        fs::read_to_string(f.root.join("engine/src/lib.rs"))
            .unwrap()
            .replace(
                "pub fn run(early: bool) -> i32 {",
                "pub fn run(early: bool) -> Option<i32> { Some({",
            )
            .replace(
                "sink(size(extended) + n) }",
                "sink(size(extended) + n) }) }",
            ),
    )
    .unwrap();
    fs::write(
        f.mods.join("alpha/horizon-mixin.toml"),
        r#"
[mod]
id="alpha"
version="1"
[[injection]]
id="overwrite"
target="fixture_game::run"
kind="OVERWRITE"
helper="overwrite.rs"
"#,
    )
    .unwrap();
    fs::write(
        f.mods.join("alpha/overwrite.rs"),
        "fn run() -> Option<i32> { crate::mark(\"overwrite\"); Some(7) }",
    )
    .unwrap();
    fs::write(
        f.root.join("driver/src/lib.rs"),
        r#"
#[test]
fn the_body_is_replaced() {
    use fixture_game::{run, LOG};
    assert_eq!(run(false), Some(7));
    assert_eq!(*LOG.lock().unwrap(), ["overwrite"]);
}
"#,
    )
    .unwrap();
    f.run("check");
}

#[test]
fn site_hooks_leave_a_question_mark_elsewhere_alone_but_not_in_their_site() {
    let body = r#"let n = Some(2).map(|n| n + 1)?; let m = sink(n); Some(sink(m + Some(1)?))"#;
    let rows = |ordinal: usize| {
        format!(
            r#"
[mod]
id="alpha"
version="1"
[[injection]]
id="before"
target="fixture_game::run"
kind="BEFORE_CALL"
selector="sink"
require=2
ordinal={ordinal}
helper="before.rs"
"#
        )
    };
    let staged = |ordinal: usize| {
        let f = fixture(body);
        fs::write(
            f.root.join("engine/src/lib.rs"),
            fs::read_to_string(f.root.join("engine/src/lib.rs"))
                .unwrap()
                .replace(
                    "pub fn run(early: bool) -> i32 {",
                    "pub fn run(early: bool) -> Option<i32> {",
                ),
        )
        .unwrap();
        fs::write(f.mods.join("alpha/horizon-mixin.toml"), rows(ordinal)).unwrap();
        fs::write(
            f.mods.join("alpha/before.rs"),
            "fn before() { crate::mark(\"before\"); }",
        )
        .unwrap();
        f
    };
    // `sink(n)`: the `?`s are in other statements.
    let f = staged(0);
    fs::write(
        f.root.join("driver/src/lib.rs"),
        r#"
#[test]
fn before_runs_once() {
    use fixture_game::{run, LOG};
    assert_eq!(run(false), Some(4));
    assert_eq!(*LOG.lock().unwrap(), ["before"]);
}
"#,
    )
    .unwrap();
    f.run("check");
    // `sink(m + Some(1)?)`: the `?` is the site's own.
    let f = staged(1);
    let e = build_plan(&f.root, &f.mods).unwrap_err();
    assert_eq!(e.code, "E_UNSUPPORTED_CONTROL_FLOW", "{e}");
    assert!(e.message.contains("selected site contains `?`"), "{e}");
}
