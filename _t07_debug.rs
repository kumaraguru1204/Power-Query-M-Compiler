#[test]
fn t07_debug() {
    let r = Engine::run_with_formula(DUMMY_JSON, r#"let
    Source = Excel.Workbook(File.Contents("dummy.xlsx"), null, true),
    f = (x) => x[Age] <> "30"
in
    Table.LastN(Source, f)"#);
    eprintln!("ERROR: {:?}", r.err().map(|e| format!("{:?}", e)));
    panic!("see above");
}
