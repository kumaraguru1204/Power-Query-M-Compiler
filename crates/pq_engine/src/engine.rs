use pq_pipeline::{build_table_from_json, Table};
use pq_lexer::Lexer;
use pq_parser::Parser;
use pq_ast::Program;
use pq_resolver::Resolver;
use pq_typechecker::TypeChecker;
use pq_formatter::{format_table, format_program};
use pq_diagnostics::{Diagnostic, Reporter};
use pq_executor::Executor;
use pq_sql::generate_sql;

#[derive(Debug)]
pub enum EngineError {
    Json(String),
    Lex(Vec<Diagnostic>),        // Changed: keep diagnostic objects
    Parse(Vec<Diagnostic>),       // Changed: keep diagnostic objects
    Execute(String),
    Diagnostics(Vec<Diagnostic>),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EngineError::Json(e)  => write!(f, "json error: {}", e),
            EngineError::Lex(diags) => {
                for d in diags { writeln!(f, "{}", d)?; }
                Ok(())
            }
            EngineError::Parse(diags) => {
                for d in diags { writeln!(f, "{}", d)?; }
                Ok(())
            }
            EngineError::Execute(e) => write!(f, "execute error: {}", e),
            EngineError::Diagnostics(diags) => {
                for d in diags { writeln!(f, "{}", d)?; }
                Ok(())
            }
        }
    }
}

pub struct EngineOutput {
    pub table:        Table,       // original raw input table
    pub result_table: Table,       // table after executing AST steps
    pub formula:      String,      // clean M formula
    pub sql:          String,      // generated SQL query
    pub program:      Program,
    pub tokens:       Vec<String>, // token list for debug display
}

pub struct Engine;

impl Engine {
    pub fn run(json: &str) -> Result<EngineOutput, EngineError> {
        let table   = build_table_from_json(json)
            .map_err(|e| EngineError::Json(e.to_string()))?;
        let formula = format_table(&table);
        Self::run_pipeline(table, formula)
    }

    pub fn run_with_formula(
        json:    &str,
        formula: &str,
    ) -> Result<EngineOutput, EngineError> {
        let table   = build_table_from_json(json)
            .map_err(|e| EngineError::Json(e.to_string()))?;
        let formula = formula.to_string();
        Self::run_pipeline(table, formula)
    }

    fn run_pipeline(
        table:   Table,
        formula: String,
    ) -> Result<EngineOutput, EngineError> {
        // ── lex ───────────────────────────────────────────────────────────
        let (tokens, token_strings) = match Lexer::new(&formula).tokenize() {
            Ok(tokens) => {
                // Build human-readable token strings for debug output.
                // Format: "[line:col]  KindDebug"
                let strings: Vec<String> = tokens
                    .iter()
                    .filter(|t| t.kind != pq_lexer::token::TokenKind::Eof)
                    .map(|t| format!("[{:>3}:{:<3}]  {:?}", t.span.line, t.span.col, t.kind))
                    .collect();
                (tokens, strings)
            }
            Err(e) => {
                return Err(EngineError::Lex(vec![e.to_diagnostic()]));
            }
        };

        // ── parse ─────────────────────────────────────────────────────────
        let mut program = match Parser::new(tokens).parse() {
            Ok(prog) => prog,
            Err(e) => {
                return Err(EngineError::Parse(vec![e.to_diagnostic()]));
            }
        };

        // ── resolve ───────────────────────────────────────────────────────
        let mut resolver = Resolver::new(&table);
        if let Err(diags) = resolver.resolve(&program) {
            return Err(EngineError::Diagnostics(diags));
        }

        // ── type check + type annotation ──────────────────────────────────
        // checker.check(&mut program) annotates every ExprNode.inferred_type
        // and Step.output_type in place, then validates for type errors.
        let mut checker = TypeChecker::new(&table);
        if let Err(diags) = checker.check(&mut program) {
            return Err(EngineError::Diagnostics(diags));
        }

        // ── clean formula ─────────────────────────────────────────────────
        let clean_formula = format_program(&program);

        let result_table = Executor::execute(&program, table.clone())
            .map_err(|e| EngineError::Execute(e.to_string()))?;

        // ── generate sql ──────────────────────────────────────────────────
        let sql = generate_sql(&program, &table);

        Ok(EngineOutput { table, result_table, formula: clean_formula, sql, program, tokens: token_strings })
    }

    pub fn render_error(error: &EngineError, source: &str) -> String {
        match error {
            EngineError::Json(e)  => format!("json error: {}\n", e),
            EngineError::Lex(diags) => Reporter::new(source).render_all(diags),
            EngineError::Parse(diags) => Reporter::new(source).render_all(diags),
            EngineError::Execute(e) => format!("execute error: {}\n", e),
            EngineError::Diagnostics(diags) => {
                Reporter::new(source).render_all(diags)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"
    {
        "source": "workbook.xlsx",
        "sheet":  "Sales",
        "rows": [
            ["Name",    "Age", "Salary",    "Active"],
            ["Alice",   "30",  "50000.50",  "true"],
            ["Bob",     "25",  "40000.00",  "false"],
            ["Charlie", "35",  "60000.75",  "true"]
        ]
    }
    "#;

    #[test]
    fn test_run_base_pipeline() {
        let output = Engine::run(JSON).unwrap();
        assert_eq!(output.table.col_count(), 4);
        assert_eq!(output.table.row_count(), 3);
        assert!(output.formula.contains("Excel.Workbook"));
    }

    #[test]
    fn test_run_with_filter() {
        let formula = r#"
        let
            Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
            Filtered = Table.SelectRows(Source, each Age > 25)
        in
            Filtered
        "#;
        let output = Engine::run_with_formula(JSON, formula).unwrap();
        assert_eq!(output.program.steps.len(), 2);
    }

    #[test]
    fn test_unknown_step_error() {
        let formula = r#"
        let
            Source   = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
            Filtered = Table.SelectRows(DoesNotExist, each Age > 25)
        in
            Filtered
        "#;
        let result = Engine::run_with_formula(JSON, formula);
        assert!(result.is_err());
    }

    #[test]
    fn test_type_mismatch_error() {
        let formula = r#"
        let
            Source  = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
            WithCol = Table.AddColumn(Source, "Bad", each Name + 1)
        in
            WithCol
        "#;
        let result = Engine::run_with_formula(JSON, formula);
        assert!(result.is_err());
    }

    #[test]
    fn test_full_pipeline() {
        let formula = r#"
        let
            Source          = Excel.Workbook(File.Contents("workbook.xlsx"), null, true),
            PromotedHeaders = Table.PromoteHeaders(Source),
            ChangedTypes    = Table.TransformColumnTypes(PromotedHeaders, {{"Name", Text.Type}, {"Age", Int64.Type}, {"Salary", Number.Type}, {"Active", Logical.Type}}),
            Filtered        = Table.SelectRows(ChangedTypes, each Age > 25),
            WithBonus       = Table.AddColumn(Filtered, "Bonus", each Salary + 1000.0),
            Removed         = Table.RemoveColumns(WithBonus, {"Active"}),
            Renamed         = Table.RenameColumns(Removed, {{"Name", "FullName"}}),
            Sorted          = Table.Sort(Renamed, {{"Age", Order.Ascending}})
        in
            Sorted
        "#;
        let output = Engine::run_with_formula(JSON, formula).unwrap();
        assert_eq!(output.program.steps.len(), 8);
        assert_eq!(output.program.output, "Sorted");
    }
}