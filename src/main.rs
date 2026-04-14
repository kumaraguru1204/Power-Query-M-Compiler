use pq_engine::Engine;

fn main() {
    // ── Input data (rows that Source table will have) ─────────────────────
    let json = r#"
    {
        "source": "workbook.xlsx",
        "sheet":  "Sales",
        "rows": [
            ["Name",    "Age", "Salary", "Active"],
            ["Alice",   "30",  "50000.50", "true"],
            ["Bob",     "25",  "40000.00", "false"],
            ["Charlie", "35",  "60000.75", "false"]
        ]
    }
    "#;

    // ── Your M formula (edit this) ────────────────────────────────────────
    let formula = r#"
            let
                Source = Excel.Workbook(File.Contents("file.xlsx"), null, true),
                Result = List.Generate(
                        () => 1,
                        each _ < [Name],
                        each _ + 1
                )
            in
                Result
    "#;

    println!("══════════════════════════════════════════════════════════");
    println!("  M Engine — Console Debug");
    println!("══════════════════════════════════════════════════════════");
    println!();

    // Tokens are printed inside the engine pipeline automatically.
    // (see pq_engine/src/engine.rs — run_pipeline prints each token)

    match Engine::run_with_formula(json, formula) {
        Ok(output) => {
            println!();
            println!("── AST ───────────────────────────────────────────────────");
            println!("{:#?}", output.program);

            println!();
            println!("── Formatted Formula ─────────────────────────────────────");
            println!("{}", output.formula);

            println!();
            println!("── Result Table ──────────────────────────────────────────");
            print!("{}", output.result_table);

            println!();
            println!("── Generated SQL ─────────────────────────────────────────");
            println!("{}", output.sql);
        }
        Err(e) => {
            println!();
            println!("── Error ─────────────────────────────────────────────────");
            eprintln!("{}", Engine::render_error(&e, formula));
        }
    }
}