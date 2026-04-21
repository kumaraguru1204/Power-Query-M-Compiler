use pq_engine::Engine;

fn main() {
    // ── Input data (rows that Source table will have) ─────────────────────
    let json = r#"
            {
        "source": "workbook.xlsx",
        "sheet": "TestSheet",
        "rows": [
            ["Name", "Age", "Dept", "Salary", "Active", "Date", "Score", "City"],
            ["Alice", "30", "HR", "50000.50", "true", "01/02/2024", "85", "Chennai"],
            ["Bob", "25", "IT", "40000.00", "false", "15/03/2024", "90", "Coimbatore"],
            ["Charlie", "35", "Finance", "60000.75", "true", "20/04/2024", "78", "Madurai"],
            ["David", "28", "IT", "45000.00", "false", "05/05/2024", "88", "Salem"]
        ]
    }
    "#;
    println!("{}", json);
    // ── Your M formula (edit this) ────────────────────────────────────────
    let formula = r#"
        let
            Result = List.Select({}, each true)
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