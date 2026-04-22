# M Engine — Project Status Report

> **Generated:** April 2026  
> **Language:** Rust (workspace of 13 crates)  
> **Purpose:** Compiler + runtime for a Power Query M–like formula language with SQL generation

---

## 1. Project Architecture

### Crate Dependency Map

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          M_Engine  (workspace root)                         │
│                                                                             │
│   src/lib.rs ─── compile_formula() ──► pq_engine::Engine::run_pipeline()   │
│   src/api.rs ─── HTTP handlers (actix-web)                                  │
│   src/bin/cli.rs ─── CLI debug mode                                         │
│   src/bin/web.rs ─── Web server (localhost:8080)                            │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   SHARED FOUNDATION CRATES                                                  │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│   │  pq_grammar  │  │   pq_types   │  │ pq_pipeline  │  │    pq_ast    │  │
│   │              │  │              │  │              │  │              │  │
│   │ • functions  │  │ • ColumnType │  │ • Table      │  │ • Expr       │  │
│   │ • keywords   │  │ • coercion   │  │ • Column     │  │ • ExprNode   │  │
│   │ • operators  │  │ • inference  │  │ • RawWorkbook│  │ • Step       │  │
│   │ • types      │  │              │  │              │  │ • StepKind   │  │
│   │ • Type       │  │              │  │              │  │ • Program    │  │
│   │ • FunctionDef│  │              │  │              │  │              │  │
│   └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘  │
│                                                                             │
│   COMPILER PIPELINE (executed in order)                                     │
│   ┌──────────┐  ┌──────────┐  ┌────────────┐  ┌───────────────┐           │
│   │ pq_lexer │→ │pq_parser │→ │pq_resolver │→ │pq_typechecker │           │
│   │          │  │          │  │            │  │               │           │
│   │ Lexer    │  │ Parser   │  │ Resolver   │  │ TypeChecker   │           │
│   │ Token    │  │ (Pratt)  │  │ Scope      │  │ step_schemas  │           │
│   │ TokenKind│  │          │  │            │  │               │           │
│   └──────────┘  └──────────┘  └────────────┘  └───────────────┘           │
│                                                                             │
│   OUTPUT GENERATORS                                                         │
│   ┌─────────────┐  ┌──────────────┐  ┌─────────────┐                      │
│   │pq_formatter │  │ pq_executor  │  │   pq_sql    │                      │
│   │             │  │              │  │             │                      │
│   │ format_     │  │ Executor     │  │ generate_   │                      │
│   │ program()   │  │ Value        │  │ sql()       │                      │
│   └─────────────┘  └──────────────┘  └─────────────┘                      │
│                                                                             │
│   ORCHESTRATOR                                                              │
│   ┌──────────────────────────────────────────────────────┐                 │
│   │  pq_engine :: Engine                                  │                 │
│   │  • run()            — from JSON data only             │                 │
│   │  • run_with_formula — from JSON data + M formula      │                 │
│   │  • run_pipeline     — lex → parse → resolve → check   │                 │
│   │                       → format → execute → SQL        │                 │
│   └──────────────────────────────────────────────────────┘                 │
│                                                                             │
│   DIAGNOSTICS (used across all pipeline stages)                             │
│   ┌──────────────────────────────────────────────────────┐                  │
│   │  pq_diagnostics                                       │                 │
│   │  • Diagnostic (error/warning/hint + code + labels)    │                 │
│   │  • Span       (line, column, byte offsets)            │                 │
│   │  • Reporter   (rich terminal-style error rendering)   │                 │
│   └──────────────────────────────────────────────────────┘                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Data Flow

### 2.1 End-to-End Compilation Pipeline

```
  ╔═══════════════════════════════════════════════════════════╗
  ║              USER INPUT                                   ║
  ║  ┌───────────────────┐    ┌─────────────────────────┐    ║
  ║  │  JSON Workbook    │    │  M Formula String       │    ║
  ║  │  {source, sheet,  │    │  let                    │    ║
  ║  │   rows: [[...]]}  │    │    Source = Excel...     │    ║
  ║  └────────┬──────────┘    │    Filtered = Table...  │    ║
  ║           │               │  in Filtered            │    ║
  ╚═══════════╪═══════════════╧────────┬────────════════╧════╝
              │                        │
              ▼                        │
  ┌───────────────────────┐            │
  │  pq_pipeline          │            │
  │  build_table_from_json│            │
  │                       │            │
  │  JSON → Table         │            │
  │  (typed columns)      │            │
  └───────────┬───────────┘            │
              │ Table                  │ formula &str
              │                        │
              │    ┌───────────────────▼───────────────────┐
              │    │         STAGE 1: LEXER                 │
              │    │         pq_lexer::Lexer                │
              │    │                                        │
              │    │  "each [Age] > 25"                     │
              │    │    → [Each, LBracket, Ident("Age"),    │
              │    │       RBracket, Gt, IntLit(25)]        │
              │    │                                        │
              │    │  Errors: E101 unterminated string      │
              │    │          E102 unexpected character      │
              │    │          E103 invalid number            │
              │    └───────────────────┬───────────────────┘
              │                        │ Vec<Token>
              │    ┌───────────────────▼───────────────────┐
              │    │         STAGE 2: PARSER                │
              │    │         pq_parser::Parser              │
              │    │                                        │
              │    │  • Pratt expression parser             │
              │    │    (operator precedence climbing)      │
              │    │  • ArgKind-driven argument dispatch    │
              │    │  • let/in program structure            │
              │    │                                        │
              │    │  Output: Program {                     │
              │    │    steps: [StepBinding { ... }],       │
              │    │    output: "FinalStep"                 │
              │    │  }                                     │
              │    │                                        │
              │    │  Errors: E2xx (unexpected token,       │
              │    │          unknown function, etc.)       │
              │    └───────────────────┬───────────────────┘
              │                        │ Program (AST)
              │    ┌───────────────────▼───────────────────┐
              │    │         STAGE 3: RESOLVER              │
              ├───►│         pq_resolver::Resolver          │
              │    │                                        │
              │    │  Validates:                            │
              │    │  ✓ step references exist in scope      │
              │    │  ✓ column names exist in table         │
              │    │  ✓ no duplicate column in AddColumn    │
              │    │  ✓ output step is defined              │
              │    │  ✓ "did you mean X?" suggestions       │
              │    │                                        │
              │    │  Errors: E301 unknown step             │
              │    │          E302 unknown column            │
              │    │          E303 duplicate column          │
              │    │          E304 unknown output step       │
              │    └───────────────────┬───────────────────┘
              │                        │ Program (names verified)
              │    ┌───────────────────▼───────────────────┐
              │    │         STAGE 4: TYPE CHECKER          │
              ├───►│         pq_typechecker::TypeChecker    │
              │    │                                        │
              │    │  • Bottom-up ExprNode annotation       │
              │    │  • Schema propagation step→step        │
              │    │  • Generic instantiation (T,U vars)    │
              │    │  • Type coercion (Int↔Float)           │
              │    │                                        │
              │    │  Annotates:                            │
              │    │    ExprNode.inferred_type = Some(...)  │
              │    │    Step.output_type = Some([(col,ty)]) │
              │    │                                        │
              │    │  Errors: E405 filter not Boolean       │
              │    │          E406 'not' needs Boolean      │
              │    │          E407 '-' needs numeric        │
              │    │          E410 incompatible list elems  │
              │    └──────┬────────────┬───────────────────┘
              │           │            │
              │           │  Program   │  (fully annotated)
              │           │            │
      ┌───────┴───────────┼────────────┼──────────────────────┐
      │                   │            │                      │
      ▼                   ▼            ▼                      ▼
  ┌──────────┐   ┌──────────────┐  ┌──────────┐   ┌──────────────┐
  │FORMATTER │   │  EXECUTOR    │  │SQL GEN   │   │  ERROR       │
  │          │   │              │  │          │   │  REPORTER    │
  │format_   │   │Executor::    │  │generate_ │   │              │
  │program() │   │execute()     │  │sql()     │   │render_all()  │
  │          │   │              │  │          │   │              │
  │ Clean M  │   │ Result Table │  │ SQL WITH │   │ Rich error   │
  │ formula  │   │ (row data)   │  │ CTEs     │   │ display      │
  └──────────┘   └──────────────┘  └──────────┘   └──────────────┘

  ╔═══════════════════════════════════════════════════════════╗
  ║              OUTPUT (EngineOutput)                        ║
  ║  • result_table  — transformed Table with actual data    ║
  ║  • formula       — reformatted clean M code              ║
  ║  • sql           — equivalent SQL query                  ║
  ║  • program       — annotated AST (for debug display)     ║
  ╚═══════════════════════════════════════════════════════════╝
```

### 2.2 Expression Type Inference (inside TypeChecker)

```
              ┌─────────────────────────────────┐
              │  ExprNode (tree traversal)       │
              │  Bottom-up, mutating in-place    │
              └────────────────┬────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
    ┌─────▼──────┐      ┌─────▼──────┐      ┌─────▼──────┐
    │ PHASE 1    │      │ PHASE 2    │      │ PHASE 3    │
    │ Recurse    │      │ Compute    │      │ Store      │
    │ children   │      │ this type  │      │ result     │
    └─────┬──────┘      └─────┬──────┘      └─────┬──────┘
          │                   │                    │
          ▼                   ▼                    ▼
  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐
  │ BinaryOp:     │  │ IntLit → Int  │  │ node.inferred │
  │   infer left  │  │ FloatLit→Flt  │  │ _type = ty    │
  │   infer right │  │ BoolLit→Bool  │  │               │
  │               │  │ StringLit→Txt │  │ return ty     │
  │ Lambda:       │  │ NullLit→Null  │  │               │
  │   bind _ type │  │               │  │               │
  │   infer body  │  │ ColumnAccess  │  │               │
  │               │  │  → schema     │  │               │
  │ FunctionCall: │  │   lookup      │  │               │
  │   infer args  │  │               │  │               │
  │   bind _ from │  │ BinaryOp →    │  │               │
  │   signature   │  │  coerce(L,R)  │  │               │
  │   infer λ     │  │               │  │               │
  └───────────────┘  │ FunctionCall→ │  │               │
                     │  grammar      │  │               │
                     │  registry     │  │               │
                     └───────────────┘  └───────────────┘
```

### 2.3 SQL Generation — CTE Chain

```
  Program.steps
  ══════════════

  Step 1: Source              ──►  WITH Source AS (
     Excel.Workbook(                   SELECT * FROM "workbook"
       File.Contents("x.xlsx"))      )
                                      │
  Step 2: PromotedHeaders     ──►    ,PromotedHeaders AS (
     Table.PromoteHeaders(Source)       SELECT * FROM Source
                                      )
                                      │
  Step 3: ChangedTypes        ──►    ,ChangedTypes AS (
     Table.TransformColumnTypes(        SELECT
       PromotedHeaders,                   CAST("Age" AS INTEGER) AS "Age",
       {{"Age",Int64.Type}})              "Name", "Salary"
                                        FROM PromotedHeaders
                                      )
                                      │
  Step 4: Filtered            ──►    ,Filtered AS (
     Table.SelectRows(                  SELECT *
       ChangedTypes,                    FROM ChangedTypes
       each [Age] > 25)                WHERE "Age" > 25
                                      )
                                      │
  Step 5: WithBonus           ──►    ,WithBonus AS (
     Table.AddColumn(                   SELECT *,
       Filtered, "Bonus",                "Salary" * 0.1 AS "Bonus"
       each [Salary] * 0.1)            FROM Filtered
                                      )
                                      │
  Step 6: Sorted              ──►  ───┘
     Table.Sort(WithBonus,         SELECT * FROM WithBonus
       {{"Age",Order.Ascending}})  ORDER BY "Age" ASC
```

---

## 3. Crate Responsibilities

| Crate | Role | Key Exports |
|:------|:-----|:------------|
| **pq_grammar** | Static function registry, type algebra, keywords, operators | `FunctionDef`, `ArgKind`, `NamespaceDef`, `Type`, `FunctionType`, `Operator` |
| **pq_types** | Runtime column type system | `ColumnType`, `coerce_types()`, `infer_type()` |
| **pq_diagnostics** | Error/warning reporting framework | `Diagnostic`, `Span`, `Reporter`, `Label` |
| **pq_lexer** | Tokenisation (source text → tokens) | `Lexer`, `Token`, `TokenKind` |
| **pq_ast** | Abstract syntax tree definitions | `Program`, `StepBinding`, `Step`, `StepKind`, `Expr`, `ExprNode` |
| **pq_parser** | Parsing (Pratt expressions + step dispatch) | `Parser::parse()` |
| **pq_pipeline** | Data model, JSON workbook loading | `Table`, `Column`, `build_table_from_json()` |
| **pq_resolver** | Scope validation, name resolution | `Resolver::resolve()`, `Scope` |
| **pq_typechecker** | Type inference + annotation + validation | `TypeChecker::check()` |
| **pq_formatter** | M formula pretty-printer (AST → clean M) | `format_program()`, `format_expr()` |
| **pq_executor** | Runtime evaluation (AST → transformed table) | `Executor::execute()`, `Value` |
| **pq_sql** | SQL CTE generation (AST → SQL query) | `generate_sql()` |
| **pq_engine** | Orchestrator — ties all stages together | `Engine`, `EngineOutput`, `EngineError` |

---

## 4. Function Completion Status

### Legend

| Symbol | Meaning |
|:------:|:--------|
| ✅ | **Full** — all pipeline stages work correctly |
| ⚠️ | **Partial** — mostly correct, known limitations exist |
| 🔲 | **Passthrough / Stub** — parsed OK but execution forwards input unchanged or returns Null |
| ❌ | **Not Implemented** — no grammar entry |

---

### 4.1 Core Step Functions (dedicated `StepKind` variant)

These functions have first-class support across **every** pipeline stage: parser, resolver, type-checker, executor, SQL generator, and formatter.

```
┌────────────────────────────────────┬───────────┬───────────┬──────────────────────────────────┐
│ Function                           │ Syntactic │ Semantic  │ Notes                            │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Excel.Workbook                     │    ✅     │    ✅     │ File.Contents(path) desugared;   │
│                                    │           │           │ useHeaders/delayTypes opt bool   │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.PromoteHeaders               │    ✅     │    ✅     │ Schema passthrough               │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.TransformColumnTypes         │    ✅     │    ✅     │ CAST in SQL; schema updated      │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.SelectRows                   │    ✅     │    ✅     │ each→lambda; SQL WHERE           │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.AddColumn                    │    ✅     │    ✅     │ 2 overloads; schema grows        │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.RemoveColumns                │    ✅     │    ✅     │ Schema shrinks; SQL survivors    │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.RenameColumns                │    ✅     │    ✅     │ SQL col AS new_col               │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.Sort                         │    ✅     │    ✅     │ ORDER BY hoisted to final query  │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.TransformColumns             │    ✅     │    ✅     │ Per-col lambda + optional type   │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ Table.Group                        │    ✅     │    ⚠️     │ Executor uses first-row proxy;   │
│                                    │           │           │ List.Sum/Count not wired yet     │
├────────────────────────────────────┼───────────┼───────────┼──────────────────────────────────┤
│ List.Transform (as step)           │    ✅     │    ⚠️     │ Inner List.* builtins → Null     │
└────────────────────────────────────┴───────────┴───────────┴──────────────────────────────────┘

  ✅ Fully complete: 9/11       ⚠️ Partial: 2/11
```

---

### 4.2 Grammar-Registered Functions → Passthrough

These are **syntactically complete** — the grammar has their `ArgKind` hints, the parser handles arguments, the formatter round-trips them, and the resolver validates step references. **Execution** simply forwards the input table unchanged.

#### Table — Construction

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.FromColumns` | ✅ | 🔲 |
| `Table.FromList` | ✅ | 🔲 |
| `Table.FromRecords` | ✅ | 🔲 |
| `Table.FromRows` | ✅ | 🔲 |
| `Table.FromValue` | ✅ | 🔲 |

#### Table — Conversion

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.ToColumns` | ✅ | 🔲 |
| `Table.ToList` | ✅ | 🔲 |
| `Table.ToRecords` | ✅ | 🔲 |
| `Table.ToRows` | ✅ | 🔲 |

#### Table — Information

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.ApproximateRowCount` | ✅ | 🔲 |
| `Table.ColumnCount` | ✅ | 🔲 |
| `Table.ColumnNames` | ✅ | 🔲 |
| `Table.ColumnsOfType` | ✅ | 🔲 |
| `Table.IsEmpty` | ✅ | 🔲 |
| `Table.IsDistinct` | ✅ | 🔲 |
| `Table.PartitionValues` | ✅ | 🔲 |
| `Table.Profile` | ✅ | 🔲 |
| `Table.RowCount` | ✅ | 🔲 |
| `Table.Schema` | ✅ | 🔲 |

#### Table — Row Operations

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.AlternateRows` | ✅ | 🔲 |
| `Table.Combine` | ✅ | 🔲 |
| `Table.FindText` | ✅ | 🔲 |
| `Table.First` | ✅ | 🔲 |
| `Table.FirstN` | ✅ | 🔲 |
| `Table.FirstValue` | ✅ | 🔲 |
| `Table.FromPartitions` | ✅ | 🔲 |
| `Table.InsertRows` | ✅ | 🔲 |
| `Table.Last` | ✅ | 🔲 |
| `Table.LastN` | ✅ | 🔲 |
| `Table.MatchesAllRows` | ✅ | 🔲 |
| `Table.MatchesAnyRows` | ✅ | 🔲 |
| `Table.Partition` | ✅ | 🔲 |
| `Table.Range` | ✅ | 🔲 |
| `Table.RemoveFirstN` | ✅ | 🔲 |
| `Table.RemoveLastN` | ✅ | 🔲 |
| `Table.RemoveRows` | ✅ | 🔲 |
| `Table.RemoveRowsWithErrors` | ✅ | 🔲 |
| `Table.Repeat` | ✅ | 🔲 |
| `Table.ReplaceRows` | ✅ | 🔲 |
| `Table.ReverseRows` | ✅ | 🔲 |
| `Table.SelectRowsWithErrors` | ✅ | 🔲 |
| `Table.SingleRow` | ✅ | 🔲 |
| `Table.Skip` | ✅ | 🔲 |
| `Table.SplitAt` | ✅ | 🔲 |

#### Table — Column Operations

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.Column` | ✅ | 🔲 |
| `Table.DemoteHeaders` | ✅ | 🔲 |
| `Table.DuplicateColumn` | ✅ | 🔲 |
| `Table.HasColumns` | ✅ | 🔲 |
| `Table.Pivot` | ✅ | 🔲 |
| `Table.PrefixColumns` | ✅ | 🔲 |
| `Table.ReorderColumns` | ✅ | 🔲 |
| `Table.TransformColumnNames` | ✅ | 🔲 |
| `Table.Unpivot` | ✅ | 🔲 |
| `Table.UnpivotOtherColumns` | ✅ | 🔲 |

#### Table — Transformation

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.AddFuzzyClusterColumn` | ✅ | 🔲 |
| `Table.AddIndexColumn` | ✅ | 🔲 |
| `Table.AddJoinColumn` | ✅ | 🔲 |
| `Table.AddKey` | ✅ | 🔲 |
| `Table.AggregateTableColumn` | ✅ | 🔲 |
| `Table.CombineColumns` | ✅ | 🔲 |
| `Table.CombineColumnsToRecord` | ✅ | 🔲 |
| `Table.ExpandListColumn` | ✅ | 🔲 |
| `Table.ExpandRecordColumn` | ✅ | 🔲 |
| `Table.ExpandTableColumn` | ✅ | 🔲 |
| `Table.FillDown` | ✅ | 🔲 |
| `Table.FillUp` | ✅ | 🔲 |
| `Table.FuzzyGroup` | ✅ | 🔲 |
| `Table.FuzzyJoin` | ✅ | 🔲 |
| `Table.FuzzyNestedJoin` | ✅ | 🔲 |
| `Table.Join` | ✅ | 🔲 |
| `Table.Keys` | ✅ | 🔲 |
| `Table.NestedJoin` | ✅ | 🔲 |
| `Table.PartitionKey` | ✅ | 🔲 |
| `Table.ReplaceErrorValues` | ✅ | 🔲 |
| `Table.ReplaceKeys` | ✅ | 🔲 |
| `Table.ReplaceMatchingRows` | ✅ | 🔲 |
| `Table.ReplacePartitionKey` | ✅ | 🔲 |
| `Table.ReplaceValue` | ✅ | 🔲 |
| `Table.Split` | ✅ | 🔲 |
| `Table.SplitColumn` | ✅ | 🔲 |
| `Table.TransformRows` | ✅ | 🔲 |
| `Table.Transpose` | ✅ | 🔲 |

#### Table — Membership

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.Contains` | ✅ | 🔲 |
| `Table.ContainsAll` | ✅ | 🔲 |
| `Table.ContainsAny` | ✅ | 🔲 |
| `Table.Distinct` | ✅ | 🔲 |
| `Table.PositionOf` | ✅ | 🔲 |
| `Table.PositionOfAny` | ✅ | 🔲 |
| `Table.RemoveMatchingRows` | ✅ | 🔲 |

#### Table — Ordering

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.AddRankColumn` | ✅ | 🔲 |
| `Table.Max` | ✅ | 🔲 |
| `Table.MaxN` | ✅ | 🔲 |
| `Table.Min` | ✅ | 🔲 |
| `Table.MinN` | ✅ | 🔲 |

#### Table — Pass-Through Utilities

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Table.Buffer` | ✅ | 🔲 |
| `Table.ConformToPageReader` | ✅ | 🔲 |
| `Table.StopFolding` | ✅ | 🔲 |

#### Tables Namespace

| Function | Syntactic | Semantic |
|:---------|:---------:|:---------:|
| `Tables.GetRelationships` | ✅ | 🔲 |

---

### 4.3 Expression-Context Functions

These appear inside `each` lambdas / `AddColumn` expressions as `Expr::FunctionCall` nodes. The grammar has full signatures; the type-checker infers return types via the registry; the executor dispatches via `call_function()`.

#### Text Functions

| Function | Grammar | Type-Check | Executor | Status |
|:---------|:-------:|:----------:|:--------:|:------:|
| `Text.Upper` | ✅ | ✅ | ✅ | ✅ Full |
| `Text.Lower` | ✅ | ✅ | ✅ | ✅ Full |
| `Text.Trim` | ✅ | ✅ | ✅ | ✅ Full |
| `Text.Length` | ✅ | ✅ | ✅ | ✅ Full |
| `Text.From` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.TrimStart` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.TrimEnd` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.PadStart` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.PadEnd` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.Contains` | ✅ | ✅ | ✅ | ✅ Full — nullable text → null, `Comparer.OrdinalIgnoreCase` supported; other comparers fall back to ordinal |
| `Text.StartsWith` | ✅ | ✅ | ✅ | ✅ Full — nullable text → null, `Comparer.OrdinalIgnoreCase` supported; other comparers fall back to ordinal |
| `Text.EndsWith` | ✅ | ✅ | ✅ | ✅ Full — nullable text → null, `Comparer.OrdinalIgnoreCase` supported; other comparers fall back to ordinal |
| `Text.Range` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.Replace` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.Split` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Text.Combine` | ✅ | ✅ | 🔲 | ⚠️ Stub |

#### Number Functions

| Function | Grammar | Type-Check | Executor | Status |
|:---------|:-------:|:----------:|:--------:|:------:|
| `Number.From` | ✅ | ✅ | ✅ | ✅ Full |
| `Number.Round` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.RoundUp` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.RoundDown` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.Abs` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.Sqrt` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.Power` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.Log` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.Mod` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Number.Sign` | ✅ | ✅ | 🔲 | ⚠️ Stub |

#### Logical Functions

| Function | Grammar | Type-Check | Executor | Status |
|:---------|:-------:|:----------:|:--------:|:------:|
| `Logical.From` | ✅ | ✅ | ✅ | ✅ Full |
| `Logical.Not` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Logical.And` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Logical.Or` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `Logical.Xor` | ✅ | ✅ | 🔲 | ⚠️ Stub |

#### List Functions (expression context)

| Function | Grammar | Type-Check | Executor | Status |
|:---------|:-------:|:----------:|:--------:|:------:|
| `List.Transform` *(step)* | ✅ | ✅ | ✅ | ✅ Full — `each`, bare fn ref (`Number.From`), explicit lambda (`(x) => …`), record-list input (`each [Field]`), nested-list input (`each List.Sum(_)`) |
| `List.Select` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Difference` | ✅ | ✅ | ✅ | ✅ Full (equationCriteria ignored at runtime — default equality only; SQL uses set semantics) |
| `List.Intersect` | ✅ | ✅ | ✅ | ✅ Full (equationCriteria ignored at runtime; SQL INTERSECT used for literal lists, runtime handles general case) |
| `List.Count` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Sum` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Average` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Min` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Max` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.First` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Last` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Single` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Contains` | ✅ | ✅ | ✅ | ✅ Full (equationCriteria falls back to default equality) |
| `List.Distinct` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Reverse` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Sort` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Accumulate` | ✅ | ✅ | 🔲 | ⚠️ Stub |
| `List.Range` | ✅ | ✅ | 🔲 | ⚠️ Stub |

---

### 4.4 Summary Counts

```
╔════════════════════════════════════════════════╦═══════╗
║ Category                                       ║ Count ║
╠════════════════════════════════════════════════╬═══════╣
║ Core steps — fully complete (all stages)       ║   9   ║
║ Core steps — partial (known executor gaps)     ║   2   ║
║ Grammar-registered — syntactic only (pass-thru)║  ~80  ║
║ Expr functions — executor fully wired          ║   7   ║
║ Expr functions — grammar + TC only (stub exec) ║  ~29  ║
╠════════════════════════════════════════════════╬═══════╣
║ Total function definitions in registry         ║ ~120  ║
╚════════════════════════════════════════════════╩═══════╝
```

```
  Completion by pipeline stage:

  Grammar Registry  ████████████████████████████████████████  100%  (~120 functions)
  Parser (syntax)   ████████████████████████████████████████  100%  (all registered funcs parseable)
  Type Checker      ████████████████████████████████████████  100%  (all sigs in registry used)
  Formatter         ████████████████████████████████████████  100%  (all StepKinds round-trip)
  Resolver          ██████████████░░░░░░░░░░░░░░░░░░░░░░░░░   35%  (11 StepKinds + passthrough)
  Executor          ████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   15%  (11 steps + 7 expr builtins)
  SQL Generator     ████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   15%  (11 steps)
```

---

## 5. Expression Language

### Operators (all fully implemented: lexer → parser → type-checker → executor)

```
  ┌─────────────────┬──────────────┬──────────────────────┐
  │ Category        │ Operators    │ Precedence (low→high)│
  ├─────────────────┼──────────────┼──────────────────────┤
  │ Logical         │ or           │ 1-2                  │
  │                 │ and          │ 3-4                  │
  ├─────────────────┼──────────────┼──────────────────────┤
  │ Comparison      │ = <> > < >=  │ 5-6                  │
  │                 │ <=           │                      │
  ├─────────────────┼──────────────┼──────────────────────┤
  │ Additive /      │ + - &        │ 7-8                  │
  │ Concatenation   │              │                      │
  ├─────────────────┼──────────────┼──────────────────────┤
  │ Multiplicative  │ * /          │ 9-10                 │
  ├─────────────────┼──────────────┼──────────────────────┤
  │ Unary (prefix)  │ not  -       │ 11 (highest)         │
  └─────────────────┴──────────────┴──────────────────────┘
```

### Literals (all fully implemented)

| Literal | Examples | Runtime Type |
|:--------|:---------|:-------------|
| Integer | `42`, `-7` | `ColumnType::Integer` |
| Float | `3.14`, `0.5` | `ColumnType::Float` |
| Boolean | `true`, `false` | `ColumnType::Boolean` |
| String | `"hello"` | `ColumnType::Text` |
| Null | `null` | `ColumnType::Null` |

### Expression Forms (all parseable, type-checked, executable)

| Form | Syntax | AST Node |
|:-----|:-------|:---------|
| Column access | `[ColumnName]` | `Expr::ColumnAccess` |
| Each sugar | `each [Age] > 25` | `Expr::Lambda { param: "_", body }` |
| Explicit lambda | `(x) => x > 0` | `Expr::Lambda { param: "x", body }` |
| Function call | `Text.Length([Name])` | `Expr::FunctionCall { name, args }` |
| List literal | `{1, 2, 3}` | `Expr::List(items)` |
| Record literal | `[Name = "Alice", Age = 30]` | `Expr::Record(fields)` |
| Binary op | `[Age] > 25 and [Active]` | `Expr::BinaryOp { left, op, right }` |
| Unary op | `not [Active]`, `-[Amount]` | `Expr::UnaryOp { op, operand }` |
| Identifier | `Source`, `Order.Ascending` | `Expr::Identifier(name)` |

---

## 6. Diagnostic System

### Error Code Ranges

```
  ┌──────────┬──────────────────┬────────────────────────────────────────┐
  │ Code     │ Phase            │ Description                            │
  ├──────────┼──────────────────┼────────────────────────────────────────┤
  │ E101     │ Lexer            │ Unterminated string literal             │
  │ E102     │ Lexer            │ Unexpected character                    │
  │ E103     │ Lexer            │ Invalid number literal                  │
  ├──────────┼──────────────────┼────────────────────────────────────────┤
  │ E2xx     │ Parser           │ Unexpected token                       │
  │          │                  │ Unknown function name                   │
  │          │                  │ Unknown sort order                      │
  ├──────────┼──────────────────┼────────────────────────────────────────┤
  │ E301     │ Resolver         │ Unknown step reference                  │
  │ E302     │ Resolver         │ Unknown column name                    │
  │ E303     │ Resolver         │ Duplicate column (AddColumn)           │
  │ E304     │ Resolver         │ Output step not defined                │
  ├──────────┼──────────────────┼────────────────────────────────────────┤
  │ E405     │ Type Checker     │ Filter condition not Boolean           │
  │ E406     │ Type Checker     │ 'not' requires Boolean operand         │
  │ E407     │ Type Checker     │ Unary '-' requires numeric operand     │
  │ E410     │ Type Checker     │ Incompatible list element types        │
  └──────────┴──────────────────┴────────────────────────────────────────┘
```

### Diagnostic Features

- **Source spans** — every error points to exact line and column
- **Underlined labels** — multiple labels per diagnostic for context
- **Fuzzy suggestions** — edit-distance ≤ 3 triggers "did you mean '…'?"
- **Available columns** — listed when no close match found
- **Severity levels** — Error, Warning, Hint

### Example Error Output

```
  error[E302]: unknown column 'Agee'
    --> formula:3:42
     |
   3 |     Filtered = Table.SelectRows(Source, each [Agee] > 25)
     |                                              ^^^^^^ column 'Agee' does not exist
     |
     = help: did you mean 'Age'?
```

---

## 7. Known Gaps and Recommended Next Steps

### Priority 1 — High-Impact Executor Gaps

```
  ┌──────────────────────────────────────────────────────────────────┐
  │  Table.Group aggregation                                         │
  │  ────────────────────────                                        │
  │  Current: uses first-row proxy for each aggregate                │
  │  Needed:  wire List.Sum, List.Count, List.Average,               │
  │           List.Min, List.Max into call_function()                │
  │           then collect all group-row values before reducing      │
  │                                                                  │
  │  Effort: ~100 lines in pq_executor/executor.rs                  │
  └──────────────────────────────────────────────────────────────────┘
```

### Priority 2 — Expression Builtins (low effort, high value)

```
  Each is a single match arm in Executor::call_function()

  ┌─────────────────────┬────────────────────────────────────┐
  │ Function            │ Implementation                      │
  ├─────────────────────┼────────────────────────────────────┤
  │ Text.Contains       │ s.contains(sub)                    │
  │ Text.Replace        │ s.replace(old, new)                │
  │ Text.StartsWith     │ s.starts_with(prefix)              │
  │ Text.EndsWith       │ s.ends_with(suffix)                │
  │ Text.From           │ v.to_raw_string()                  │
  │ Number.Abs          │ n.abs()                            │
  │ Number.Round        │ (n * 10^d).round() / 10^d         │
  │ Number.Mod          │ n % d                              │
  │ List.Count          │ items.len()                        │
  │ List.Sum            │ items.iter().sum()                 │
  └─────────────────────┴────────────────────────────────────┘
```

### Priority 3 — Simple Step Promotions (Passthrough → StepKind)

```
  Candidates (simple slice/filter ops):

  • Table.FirstN(prev, n)        → select_rows(&(0..n))
  • Table.LastN(prev, n)         → select_rows(&(total-n..total))
  • Table.Skip(prev, n)          → select_rows(&(n..total))
  • Table.Range(prev, off, cnt)  → select_rows(&(off..off+cnt))
  • Table.Distinct(prev, cols)   → deduplicate by column values
  • Table.ReverseRows(prev)      → reverse index order
  • Table.FillDown(prev, cols)   → forward-fill nulls
```

### Priority 4 — Join / Multi-Table Operations

```
  • Table.Join           → SQL INNER/LEFT/RIGHT/FULL JOIN
  • Table.NestedJoin     → sub-table per row
  • Table.Combine        → UNION ALL

  These require multi-table executor context + SQL JOIN clauses.
```

### Priority 5 — Parser Resilience

```
  • Error recovery: on parse failure, skip to next ',' or 'in'
    to report multiple errors per compilation
  • Currently: bails on first error
```

---

## 8. Interface Modes

```
  ┌─ WEB INTERFACE ──────────────────────────────────────────────┐
  │  cargo run --bin web                                          │
  │  http://localhost:8080                                        │
  │                                                               │
  │  ┌─────────────────────────────────────────────────────────┐ │
  │  │  Monaco Code Editor                                      │ │
  │  │  • Syntax highlighting                                   │ │
  │  │  • Ctrl+Enter to compile                                 │ │
  │  └─────────────────────────────────────────────────────────┘ │
  │                                                               │
  │  Tabs: [ Output | Errors | SQL | Formatted | Debug ]          │
  │                                                               │
  │  • Output     — transformed table result                     │
  │  • Errors     — diagnostic messages with line/col info       │
  │  • SQL        — generated SQL query                          │
  │  • Formatted  — clean M formula                              │
  │  • Debug      — AST + token dump                             │
  └───────────────────────────────────────────────────────────────┘

  ┌─ CLI DEBUG MODE ─────────────────────────────────────────────┐
  │  cargo run --bin cli -- 'formula' --debug                    │
  │  cargo run --bin cli -- -f formula.pq --debug                │
  │                                                               │
  │  Console output:                                              │
  │  • Tokens (every token with span)                            │
  │  • AST    (Program {:#?} debug dump)                         │
  │  • Clean formula                                              │
  │  • SQL query                                                  │
  │  • Result table                                               │
  │  • Error diagnostics with source underlining                 │
  └───────────────────────────────────────────────────────────────┘
```

---

*End of report.*
