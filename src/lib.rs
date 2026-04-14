// Shared library for CLI and web

pub mod api;

use pq_engine::Engine;
use pq_diagnostics::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompileRequest {
    pub formula: String,
    pub data: Option<String>,
    pub debug: Option<bool>,  // Enable verbose output
}

#[derive(Debug, Serialize)]
pub struct DetailedError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    /// Number of characters the error span covers (used for highlighting).
    pub span_len: Option<usize>,
    pub context: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompileResponse {
    pub success: bool,
    pub result: Option<String>,
    pub errors: Vec<DetailedError>,
    pub warnings: Vec<String>,
    pub formatted_code: Option<String>,
    pub sql: Option<String>,
    pub tokens: Option<Vec<String>>,      // For debug
    pub ast: Option<String>,               // For debug
}

/// Main compile function used by both CLI and web
pub fn compile_formula(req: CompileRequest) -> CompileResponse {
    let CompileRequest { formula, data, debug } = req;
    let debug = debug.unwrap_or(false);
    
    let json_data = data.unwrap_or_else(|| {
        r#"{
            "source": "data.xlsx",
            "sheet": "Sheet1",
            "rows": [
                ["Name", "Age", "Salary"],
                ["Alice", "30", "50000"],
                ["Bob", "25", "40000"],
                ["Charlie", "35", "60000"]
            ]
        }"#.to_string()
    });
    
    match Engine::run_with_formula(&json_data, &formula) {
        Ok(output) => {
            if debug {
                eprintln!("✓ Compilation successful");
                eprintln!("AST: {:#?}", output.program);
            }
            
            CompileResponse {
                success: true,
                result: Some(output.result_table.to_string()),
                errors: vec![],
                warnings: vec![],
                formatted_code: Some(output.formula),
                sql: Some(output.sql),
                tokens: if debug { Some(output.tokens) } else { None },
                ast: if debug { Some(format!("{:#?}", output.program)) } else { None },
            }
        }
        Err(e) => {
            let errors = extract_diagnostics_from_error(&e, &formula);
            
            if debug {
                eprintln!("✗ Compilation failed");
                for err in &errors {
                    if let Some(line) = err.line {
                        eprintln!("  Line {}, Col {}: {}", line, err.column.unwrap_or(0), err.message);
                    } else {
                        eprintln!("  {}", err.message);
                    }
                }
            }
            
            CompileResponse {
                success: false,
                result: None,
                errors,
                warnings: vec![],
                formatted_code: None,
                sql: None,
                tokens: None,
                ast: None,
            }
        }
    }
}

/// Extract diagnostic errors with proper line/column info
fn extract_diagnostics_from_error(error: &pq_engine::EngineError, formula: &str) -> Vec<DetailedError> {
    match error {
        pq_engine::EngineError::Json(e) => {
            vec![DetailedError {
                message: format!("json error: {}", e),
                line: None,
                column: None,
                span_len: None,
                context: None,
            }]
        }
        pq_engine::EngineError::Lex(diags) | 
        pq_engine::EngineError::Parse(diags) | 
        pq_engine::EngineError::Diagnostics(diags) => {
            convert_diagnostics(diags, formula)
        }
        pq_engine::EngineError::Execute(e) => {
            vec![DetailedError {
                message: format!("execute error: {}", e),
                line: None,
                column: None,
                span_len: None,
                context: None,
            }]
        }
    }
}

/// Convert Diagnostic objects to DetailedError with context
fn convert_diagnostics(diagnostics: &[Diagnostic], formula: &str) -> Vec<DetailedError> {
    diagnostics.iter().map(|diag| {
        // Get the first label with location info
        let (line, column, span_len, context) = if let Some(label) = diag.labels.first() {
            let span = &label.span;
            if span.is_dummy() {
                (None, None, None, None)
            } else {
                // Extract the actual line from the formula (keep indentation for display)
                let full_line = formula
                    .lines()
                    .nth(span.line.saturating_sub(1))
                    .unwrap_or("");
                
                (
                    Some(span.line),
                    Some(span.col),
                    Some(span.len().max(1)),
                    if full_line.is_empty() { None } else { Some(full_line.to_string()) }
                )
            }
        } else {
            (None, None, None, None)
        };
        
        // Build comprehensive error message
        let message = if let Some(label) = diag.labels.first() {
            format!("{}: {}", diag.message, label.message)
        } else {
            diag.message.clone()
        };
        
        DetailedError {
            message,
            line,
            column,
            span_len,
            context,
        }
    }).collect()
}
