#[test]
fn export_plugin() {
    let t = trybuild::TestCases::new();
    t.pass("tests/cases/pass_*.rs");
    t.compile_fail("tests/cases/fail_*.rs");
}
