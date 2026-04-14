/// CLI Binary: Run formula from command line with full debug output
/// Usage: cargo run --bin cli <formula> [--data <json>] [--debug]

use std::env;
use std::fs;
use m_engine::{CompileRequest, compile_formula};

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_usage();
        return;
    }
    
    let mut formula = String::new();
    let mut data = None;
    let mut debug = false;
    let mut file_input = false;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--data" => {
                if i + 1 < args.len() {
                    data = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --data requires an argument");
                    return;
                }
            }
            "--file" | "-f" => {
                if i + 1 < args.len() {
                    match fs::read_to_string(&args[i + 1]) {
                        Ok(content) => formula = content,
                        Err(e) => {
                            eprintln!("Error reading file: {}", e);
                            return;
                        }
                    }
                    i += 2;
                    file_input = true;
                } else {
                    eprintln!("Error: --file requires an argument");
                    return;
                }
            }
            "--debug" => {
                debug = true;
                i += 1;
            }
            _ => {
                if !file_input {
                    formula = args[i].clone();
                }
                i += 1;
            }
        }
    }
    
    if formula.is_empty() {
        eprintln!("Error: no formula provided");
        print_usage();
        return;
    }
    
    println!("🔧 M Engine CLI");
    println!("{}", "=".repeat(60));
    
    if debug {
        println!("📋 Formula:");
        println!("{}", formula);
        println!("{}", "-".repeat(60));
    }
    
    let response = compile_formula(CompileRequest {
        formula,
        data,
        debug: Some(debug),
    });
    
    if response.success {
        println!("✓ Compilation successful!");
        println!("{}", "-".repeat(60));
        
        if let Some(formatted) = response.formatted_code {
            println!("📝 Formatted Code:");
            println!("{}", formatted);
            println!("{}", "-".repeat(60));
        }
        
        if let Some(result) = response.result {
            println!("📊 Result Table:");
            println!("{}", result);
            println!("{}", "-".repeat(60));
        }
        
        if let Some(sql) = response.sql {
            println!("🔍 Generated SQL:");
            println!("{}", sql);
            println!("{}", "-".repeat(60));
        }
        
        if debug {
            if let Some(ast) = response.ast {
                println!("🌳 AST:");
                println!("{}", ast);
                println!("{}", "-".repeat(60));
            }
            if let Some(tokens) = response.tokens {
                println!("🔤 Tokens:");
                for token in tokens {
                    println!("  {}", token);
                }
                println!("{}", "-".repeat(60));
            }
        }
    } else {
        println!("✗ Compilation failed!");
        println!("{}", "-".repeat(60));
        for error in response.errors {
            println!("Error: {}", error.message);
            if let Some(line) = error.line {
                println!("  at line {}", line);
            }
        }
        println!("{}", "-".repeat(60));
    }
}

fn print_usage() {
    println!("Usage: cargo run --bin cli <formula> [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -f, --file <path>      Read formula from file");
    println!("  --data <json>          Provide input data as JSON");
    println!("  --debug                Show detailed output (AST, tokens)");
    println!();
    println!("Examples:");
    println!("  cargo run --bin cli 'let x = 1 in x'");
    println!("  cargo run --bin cli -f formula.pq --debug");
    println!("  cargo run --bin cli 'Table.SelectRows(t, each [Age]>25)' --data '{{...}}'");
}
