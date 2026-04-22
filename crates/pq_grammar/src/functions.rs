use std::sync::OnceLock;
use crate::types::{
    Type, FunctionType,
    SchemaTransformFn, SchemaTransformArgs, TableSchema, ColumnSchema,
    sig, sigp, fun, list, tvar, p, opt, nullable,
};

// ── ArgKind – parsing hint layer ──────────────────────────────────────────

/// Argument kinds that tell the **parser** how to read each positional
/// parameter.  They carry *no* semantic type information; that lives in
/// [`FunctionDef::signatures`].
#[derive(Debug, Clone, PartialEq)]
pub enum ArgKind {
    StringLit, StepRef, EachExpr, TypeList, ColumnList,
    RenameList, SortList, Integer, Value, RecordLit, RecordList,
    AggregateList, JoinKind, TransformList, StepRefList,
    OptInteger, OptValue, OptRecordLit, OptJoinKind,
    /// A list/step argument that may be either:
    /// - a **bare identifier** (step reference, e.g. `MyList`) — stored as `step_ref`, or
    /// - any **expression** (raw list `{1,2,3}`, function call `List.Range(…)`, etc.)
    ///   — stored in `value_args`.
    ///
    /// Used by List.* functions whose first argument can be an inline literal
    /// as well as a reference to a previously-defined step.
    StepRefOrValue,
    /// Parses `File.Contents("path")` and extracts the path string into `str_args`.
    /// Used as the first argument to `Excel.Workbook`.
    FileContentsArg,
    /// Optional nullable-bool argument: absent | `null` | `true` | `false`.
    /// Absent or `null` → `None`; `true`/`false` → `Some(bool)`.
    /// Collected into `opt_null_bool_args` in the parser state.
    OptNullableBool,
    /// Second argument to `Table.RemoveColumns`: either a bare `"string"` or a
    /// list `{"col1","col2",...}`.  Always normalised into `col_lists`.
    ColumnListOrString,
    /// Optional third argument to `Table.RemoveColumns`: `MissingField.Error`,
    /// `MissingField.Ignore`, or `MissingField.UseNull`.
    /// Stored in `opt_missing_field` in the parser state.
    OptMissingField,
    /// Second argument to `Table.ColumnsOfType`: a brace-enclosed list of bare
    /// M type expressions, e.g. `{type number, type text}`.
    /// Stored in `bare_type_list` in the parser state.
    BareTypeList,
    /// Optional third argument to `Table.TransformColumnTypes`: either a bare
    /// culture string `"fr-FR"` or a record `[Culture="fr-FR", MissingField=MissingField.X]`.
    /// Extracts into `opt_culture_str` and `opt_missing_field` in the parser state.
    OptCultureOrRecord,
}

// ── FunctionDef ───────────────────────────────────────────────────────────

/// One function definition inside a namespace.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name:             &'static str,
    pub arg_hints:        Vec<ArgKind>,
    /// All valid overloads, tried in order during semantic resolution.
    pub signatures:       Vec<FunctionType>,
    /// Schema-transform hook for functions whose output columns depend on
    /// call-site arguments.  `None` means the output schema is opaque or
    /// identical to the input.
    pub schema_transform: Option<SchemaTransformFn>,
    pub doc:              &'static str,
}

impl FunctionDef {
    pub fn qualified(&self, namespace: &str) -> String {
        format!("{}.{}", namespace, self.name)
    }
    /// Primary (first) signature.
    pub fn primary_sig(&self) -> &FunctionType {
        self.signatures.first().expect("FunctionDef must have ≥1 signature")
    }
    /// Return the first signature whose arity matches `n` supplied args.
    pub fn overload_for_arity(&self, n: usize) -> Option<&FunctionType> {
        self.signatures.iter().find(|s| s.arity_matches(n))
    }
}

// ── NamespaceDef ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NamespaceDef {
    pub name:      &'static str,
    pub functions: Vec<FunctionDef>,
}

impl NamespaceDef {
    pub fn get_function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.iter().find(|f| f.name == name)
    }
}

// ── Lazy registry ─────────────────────────────────────────────────────────

static REGISTRY_CELL: OnceLock<Vec<NamespaceDef>> = OnceLock::new();

pub fn registry() -> &'static [NamespaceDef] {
    REGISTRY_CELL.get_or_init(build_registry)
}

fn build_registry() -> Vec<NamespaceDef> {
    vec![
        NamespaceDef { name: "Excel",   functions: excel_functions()   },
        NamespaceDef { name: "Table",   functions: table_functions()   },
        NamespaceDef { name: "Tables",  functions: tables_functions()  },
        NamespaceDef { name: "List",    functions: list_functions()    },
        NamespaceDef { name: "Text",    functions: text_functions()    },
        NamespaceDef { name: "Number",  functions: number_functions()  },
        NamespaceDef { name: "Logical", functions: logical_functions() },
    ]
}

// ── Macro to build a simple FunctionDef with one signature and no schema hook ─

macro_rules! fndef {
    ($name:literal, [$($hint:expr),*], $sig:expr, $doc:literal) => {
        FunctionDef {
            name:             $name,
            arg_hints:        vec![$($hint),*],
            signatures:       vec![$sig],
            schema_transform: None,
            doc:              $doc,
        }
    };
}

// ── Schema-transform implementations ─────────────────────────────────────

/// Output schema = input schema + one new column whose type comes from the
/// `each` expression return type (stored in `args.fn_return_type`).
fn schema_add_column(args: &SchemaTransformArgs) -> Option<TableSchema> {
    let schema   = args.input_schema.as_ref()?;
    let col_name = args.str_args.first()?;
    let col_ty   = args.fn_return_type.clone().unwrap_or(Type::Any);
    let mut out  = schema.clone();
    out.columns.push(ColumnSchema::new(col_name.clone(), col_ty));
    Some(out)
}

/// Output schema = input schema − the listed columns.
fn schema_remove_columns(args: &SchemaTransformArgs) -> Option<TableSchema> {
    Some(args.input_schema.as_ref()?.clone().remove_columns(&args.str_args))
}

/// Output schema = input schema with columns renamed per `rename_pairs`.
fn schema_rename_columns(args: &SchemaTransformArgs) -> Option<TableSchema> {
    let mut out = args.input_schema.as_ref()?.clone();
    for (old, new) in &args.rename_pairs {
        out = out.rename_column(old, new);
    }
    Some(out)
}

/// Output schema = subset of input schema, in the caller-supplied order.
fn schema_select_columns(args: &SchemaTransformArgs) -> Option<TableSchema> {
    let schema = args.input_schema.as_ref()?;
    let names: Vec<&str> = args.str_args.iter().map(String::as_str).collect();
    Some(schema.clone().select_columns(&names))
}

/// Output schema = input schema with column types updated per `type_pairs`.
fn schema_transform_column_types(args: &SchemaTransformArgs) -> Option<TableSchema> {
    let mut out = args.input_schema.as_ref()?.clone();
    for (col_name, new_ty) in &args.type_pairs {
        out = out.retype_column(col_name, new_ty.clone());
    }
    Some(out)
}

/// Output schema = input schema with column types updated by `type_pairs`
/// (TransformColumns uses the same mechanism as TransformColumnTypes).
fn schema_transform_columns(args: &SchemaTransformArgs) -> Option<TableSchema> {
    schema_transform_column_types(args)
}

/// Output schema = input schema unchanged (row-filter, sort, passthrough ops).
fn schema_passthrough(args: &SchemaTransformArgs) -> Option<TableSchema> {
    args.input_schema.clone()
}

/// Output schema = input schema + one new column named `str_args[0]` with
/// type `Number` (index column is always integral).
fn schema_add_index_column(args: &SchemaTransformArgs) -> Option<TableSchema> {
    let schema   = args.input_schema.as_ref()?;
    let col_name = args.str_args.first()?;
    let mut out  = schema.clone();
    out.columns.push(ColumnSchema::new(col_name.clone(), Type::Number));
    Some(out)
}

/// Output schema = input schema + duplicated column.
/// `str_args[0]` = source column, `str_args[1]` = new column name.
fn schema_duplicate_column(args: &SchemaTransformArgs) -> Option<TableSchema> {
    let schema  = args.input_schema.as_ref()?;
    let src_col = args.str_args.first()?;
    let new_col = args.str_args.get(1)?;
    let src_ty  = schema.get_column(src_col)?.ty.clone();
    let mut out = schema.clone();
    out.columns.push(ColumnSchema::new(new_col.clone(), src_ty));
    Some(out)
}

// ── Excel namespace ───────────────────────────────────────────────────────

fn excel_functions() -> Vec<FunctionDef> { vec![
    FunctionDef {
        name:             "Workbook",
        //  Excel.Workbook(File.Contents(path), optional useHeaders, optional delayTypes)
        arg_hints:        vec![
            ArgKind::FileContentsArg,  // File.Contents("path") → extracts path
            ArgKind::OptNullableBool,  // useHeaders: null | true | false
            ArgKind::OptNullableBool,  // delayTypes: null | true | false
        ],
        signatures: vec![
            // Full form: (binary, useHeaders?, delayTypes?) -> Table
            sigp(vec![
                p(Type::Any),
                opt(Type::Any),
                opt(nullable(Type::Boolean)),
            ], Type::Table),
        ],
        schema_transform: None,
        doc: "Excel.Workbook(File.Contents(path), optional useHeaders, optional delayTypes) \
              — Returns the contents of the Excel workbook as a table.",
    },
]}

// ── Table namespace ───────────────────────────────────────────────────────

fn table_functions() -> Vec<FunctionDef> { vec![

    // Construction ────────────────────────────────────────────────────────
    fndef!("FromColumns", [ArgKind::Value, ArgKind::ColumnList],
        sig(vec![list(list(Type::Any)), list(Type::Text)], Type::Table),
        "Table.FromColumns({list1,...},{col,...})"),
    FunctionDef {
        name:      "FromList",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], Type::Table),
            sig(vec![list(tvar("T")), fun(vec![tvar("T")], Type::Record)], Type::Table),
        ],
        schema_transform: None,
        doc: "Table.FromList(list, optional each splitter)",
    },
    FunctionDef {
        name:      "FromRecords",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(Type::Record)], Type::Table)],
        schema_transform: None,
        doc: "Table.FromRecords(records)",
    },
    fndef!("FromRows", [ArgKind::Value, ArgKind::ColumnList],
        sig(vec![list(Type::Any), list(Type::Text)], Type::Table),
        "Table.FromRows({row,...},{col,...})"),
    FunctionDef {
        name:      "FromValue",
        arg_hints: vec![ArgKind::Value, ArgKind::OptRecordLit],
        signatures: vec![sig(vec![Type::Any], Type::Table)],
        schema_transform: None,
        doc: "Table.FromValue(value, optional [DefaultFieldName=...])",
    },

    // Conversions ─────────────────────────────────────────────────────────
    fndef!("ToColumns", [ArgKind::StepRef],
        sig(vec![Type::Table], list(list(Type::Any))),
        "Table.ToColumns(prev)"),
    FunctionDef {
        name:      "ToList",
        arg_hints: vec![ArgKind::StepRef, ArgKind::OptValue],
        signatures: vec![
            sig(vec![Type::Table], list(tvar("T"))),
            sig(vec![Type::Table, fun(vec![Type::Record], tvar("T"))], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "Table.ToList(prev, optional each combiner)",
    },
    fndef!("ToRecords", [ArgKind::StepRef],
        sig(vec![Type::Table], list(Type::Record)),
        "Table.ToRecords(prev)"),
    fndef!("ToRows", [ArgKind::StepRef],
        sig(vec![Type::Table], list(list(Type::Any))),
        "Table.ToRows(prev)"),

    // Information ─────────────────────────────────────────────────────────
    fndef!("ApproximateRowCount", [ArgKind::StepRef], sig(vec![Type::Table], Type::Number), "Table.ApproximateRowCount(prev)"),
    fndef!("ColumnCount",         [ArgKind::StepRef], sig(vec![Type::Table], Type::Number), "Table.ColumnCount(prev)"),
    fndef!("IsEmpty",             [ArgKind::StepRef], sig(vec![Type::Table], Type::Boolean), "Table.IsEmpty(prev)"),
    fndef!("PartitionValues",     [ArgKind::StepRef], sig(vec![Type::Table], Type::Any),    "Table.PartitionValues(prev)"),
    fndef!("Profile",             [ArgKind::StepRef], sig(vec![Type::Table], Type::Table),  "Table.Profile(prev)"),
    fndef!("RowCount",            [ArgKind::StepRef], sig(vec![Type::Table], Type::Number), "Table.RowCount(prev)"),
    fndef!("Schema",              [ArgKind::StepRef], sig(vec![Type::Table], Type::Table),  "Table.Schema(prev)"),

    // Row operations ──────────────────────────────────────────────────────
    fndef!("AlternateRows",
        [ArgKind::StepRef, ArgKind::Integer, ArgKind::Integer, ArgKind::Integer],
        sig(vec![Type::Table, Type::Number, Type::Number, Type::Number], Type::Table),
        "Table.AlternateRows(prev, offset, skip, take)"),
    fndef!("Combine",   [ArgKind::StepRefList], sig(vec![list(Type::Table)], Type::Table), "Table.Combine({t1,t2,...})"),
    fndef!("FindText",  [ArgKind::StepRef, ArgKind::StringLit], sig(vec![Type::Table, Type::Text], Type::Table), "Table.FindText(prev, text)"),
    fndef!("First",     [ArgKind::StepRef, ArgKind::Value],    sig(vec![Type::Table, Type::Any], Type::Record),  "Table.First(prev, default)"),
    FunctionDef {
        name:             "FirstN",
        arg_hints:        vec![ArgKind::StepRefOrValue, ArgKind::Value],
        signatures:       vec![
            sig(vec![Type::Table, Type::Number], Type::Table),
            sig(vec![Type::Table, fun(vec![Type::Record], Type::Boolean)], Type::Table),
        ],
        schema_transform: Some(schema_passthrough),
        doc:              "Table.FirstN(prev, countOrCondition) — count (number) or each predicate (take-while)",
    },
    FunctionDef {
        name:      "FirstValue",
        arg_hints: vec![ArgKind::StepRef, ArgKind::Value],
        signatures: vec![sig(vec![Type::Table, tvar("T")], tvar("T"))],
        schema_transform: None,
        doc: "Table.FirstValue(prev, default)",
    },
    fndef!("FromPartitions", [ArgKind::StringLit, ArgKind::StepRef], sig(vec![Type::Text, Type::Table], Type::Table), "Table.FromPartitions(col, partitions)"),
    fndef!("InsertRows", [ArgKind::StepRef, ArgKind::Integer, ArgKind::RecordList],
        sig(vec![Type::Table, Type::Number, list(Type::Record)], Type::Table),
        "Table.InsertRows(prev, offset, {row,...})"),
    fndef!("Last",       [ArgKind::StepRef, ArgKind::Value],   sig(vec![Type::Table, Type::Any], Type::Record),  "Table.Last(prev, default)"),
    fndef!("LastN",      [ArgKind::StepRef, ArgKind::Integer], sig(vec![Type::Table, Type::Number], Type::Table), "Table.LastN(prev, n)"),
    FunctionDef {
        name:      "MatchesAllRows",
        arg_hints: vec![ArgKind::StepRef, ArgKind::EachExpr],
        signatures: vec![sig(vec![Type::Table, fun(vec![Type::Record], Type::Boolean)], Type::Boolean)],
        schema_transform: None,
        doc: "Table.MatchesAllRows(prev, each pred)",
    },
    FunctionDef {
        name:      "MatchesAnyRows",
        arg_hints: vec![ArgKind::StepRef, ArgKind::EachExpr],
        signatures: vec![sig(vec![Type::Table, fun(vec![Type::Record], Type::Boolean)], Type::Boolean)],
        schema_transform: None,
        doc: "Table.MatchesAnyRows(prev, each pred)",
    },
    fndef!("Partition",     [ArgKind::StepRef, ArgKind::StringLit, ArgKind::Integer],
        sig(vec![Type::Table, Type::Text, Type::Number], list(Type::Table)),
        "Table.Partition(prev, col, groups)"),
    fndef!("Range",         [ArgKind::StepRef, ArgKind::Integer, ArgKind::Integer],
        sig(vec![Type::Table, Type::Number, Type::Number], Type::Table),
        "Table.Range(prev, offset, count)"),
    fndef!("RemoveFirstN",  [ArgKind::StepRef, ArgKind::Integer], sig(vec![Type::Table, Type::Number], Type::Table), "Table.RemoveFirstN(prev, n)"),
    fndef!("RemoveLastN",   [ArgKind::StepRef, ArgKind::Integer], sig(vec![Type::Table, Type::Number], Type::Table), "Table.RemoveLastN(prev, n)"),
    fndef!("RemoveRows",    [ArgKind::StepRef, ArgKind::Integer, ArgKind::Integer],
        sig(vec![Type::Table, Type::Number, Type::Number], Type::Table),
        "Table.RemoveRows(prev, offset, count)"),
    FunctionDef {
        name:      "RemoveRowsWithErrors",
        arg_hints: vec![ArgKind::StepRef, ArgKind::ColumnList],
        signatures: vec![sig(vec![Type::Table, list(Type::Text)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc: "Table.RemoveRowsWithErrors(prev, {col,...})",
    },
    fndef!("Repeat",    [ArgKind::StepRef, ArgKind::Integer],  sig(vec![Type::Table, Type::Number], Type::Table), "Table.Repeat(prev, n)"),
    fndef!("ReplaceRows", [ArgKind::StepRef, ArgKind::Integer, ArgKind::Integer, ArgKind::RecordList],
        sig(vec![Type::Table, Type::Number, Type::Number, list(Type::Record)], Type::Table),
        "Table.ReplaceRows(prev, offset, count, {row,...})"),
    FunctionDef {
        name:      "ReverseRows",
        arg_hints: vec![ArgKind::StepRef],
        signatures: vec![sig(vec![Type::Table], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc: "Table.ReverseRows(prev)",
    },
    FunctionDef {
        // (Table, (Record) → Boolean) → Table
        name:             "SelectRows",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::EachExpr],
        signatures:       vec![sig(vec![Type::Table, fun(vec![Type::Record], Type::Boolean)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc:              "Table.SelectRows(prev, each predicate)",
    },
    FunctionDef {
        name:      "SelectRowsWithErrors",
        arg_hints: vec![ArgKind::StepRef, ArgKind::ColumnList],
        signatures: vec![sig(vec![Type::Table, list(Type::Text)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc: "Table.SelectRowsWithErrors(prev, {col,...})",
    },
    fndef!("SingleRow", [ArgKind::StepRef], sig(vec![Type::Table], Type::Record), "Table.SingleRow(prev)"),
    fndef!("Skip",      [ArgKind::StepRef, ArgKind::Integer], sig(vec![Type::Table, Type::Number], Type::Table), "Table.Skip(prev, n)"),
    fndef!("SplitAt",   [ArgKind::StepRef, ArgKind::Integer],
        sig(vec![Type::Table, Type::Number], list(Type::Table)),
        "Table.SplitAt(prev, n)"),

    // Column operations ────────────────────────────────────────────────────
    FunctionDef {
        name:      "Column",
        arg_hints: vec![ArgKind::StepRef, ArgKind::StringLit],
        signatures: vec![sig(vec![Type::Table, Type::Text], list(tvar("T")))],
        schema_transform: None,
        doc: "Table.Column(prev, col) → List<T>",
    },
    fndef!("ColumnNames", [ArgKind::StepRef], sig(vec![Type::Table], list(Type::Text)), "Table.ColumnNames(prev)"),
    fndef!("ColumnsOfType", [ArgKind::StepRefOrValue, ArgKind::BareTypeList],
        sig(vec![Type::Table, list(Type::Any)], list(Type::Text)),
        "Table.ColumnsOfType(prev, {type,...})"),
    fndef!("DemoteHeaders", [ArgKind::StepRef], sig(vec![Type::Table], Type::Table), "Table.DemoteHeaders(prev)"),
    FunctionDef {
        name:             "DuplicateColumn",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::StringLit, ArgKind::StringLit],
        signatures:       vec![sig(vec![Type::Table, Type::Text, Type::Text], Type::Table)],
        schema_transform: Some(schema_duplicate_column),
        doc:              "Table.DuplicateColumn(prev, col, newCol)",
    },
    FunctionDef {
        name:             "HasColumns",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::ColumnListOrString],
        signatures:       vec![
            sig(vec![Type::Table, list(Type::Text)], Type::Boolean),
            sigp(vec![p(Type::Table), p(Type::Text)], Type::Boolean),
        ],
        schema_transform: None,
        doc:              "Table.HasColumns(prev, {col,...} or \"col\")",
    },
    fndef!("Pivot", [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit, ArgKind::StringLit],
        sig(vec![Type::Table, list(Type::Any), Type::Text, Type::Text], Type::Table),
        "Table.Pivot(prev, {val,...}, attrCol, valCol)"),
    fndef!("PrefixColumns", [ArgKind::StepRef, ArgKind::StringLit],
        sig(vec![Type::Table, Type::Text], Type::Table),
        "Table.PrefixColumns(prev, prefix)"),
    fndef!("PromoteHeaders", [ArgKind::StepRef], sig(vec![Type::Table], Type::Table), "Table.PromoteHeaders(prev)"),
    FunctionDef {
        name:             "RemoveColumns",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::ColumnListOrString, ArgKind::OptMissingField],
        signatures:       vec![
            // Table.RemoveColumns(table, {"col1",...})
            sig(vec![Type::Table, list(Type::Text)], Type::Table),
            // Table.RemoveColumns(table, "col")
            sigp(vec![p(Type::Table), p(Type::Text)], Type::Table),
            // Table.RemoveColumns(table, {"col1",...}, MissingField.X)
            sigp(vec![p(Type::Table), p(list(Type::Text)), opt(Type::Any)], Type::Table),
            // Table.RemoveColumns(table, "col", MissingField.X)
            sigp(vec![p(Type::Table), p(Type::Text), opt(Type::Any)], Type::Table),
        ],
        schema_transform: Some(schema_remove_columns),
        doc:              "Table.RemoveColumns(prev, {col,...}, optional MissingField.X)",
    },
    FunctionDef {
        name:      "ReorderColumns",
        arg_hints: vec![ArgKind::StepRef, ArgKind::ColumnList],
        signatures: vec![sig(vec![Type::Table, list(Type::Text)], Type::Table)],
        schema_transform: Some(schema_select_columns), // same logic: keep in given order
        doc: "Table.ReorderColumns(prev, {col,...})",
    },
    FunctionDef {
        name:             "RenameColumns",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::RenameList],
        signatures:       vec![sig(vec![Type::Table, list(Type::Any)], Type::Table)],
        schema_transform: Some(schema_rename_columns),
        doc:              "Table.RenameColumns(prev, {{old, new},...})",
    },
    FunctionDef {
        name:             "SelectColumns",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::ColumnListOrString, ArgKind::OptMissingField],
        signatures:       vec![
            // Table.SelectColumns(table, {"col1",...})
            sig(vec![Type::Table, list(Type::Text)], Type::Table),
            // Table.SelectColumns(table, "col")
            sigp(vec![p(Type::Table), p(Type::Text)], Type::Table),
            // Table.SelectColumns(table, {"col1",...}, MissingField.X)
            sigp(vec![p(Type::Table), p(list(Type::Text)), opt(Type::Any)], Type::Table),
            // Table.SelectColumns(table, "col", MissingField.X)
            sigp(vec![p(Type::Table), p(Type::Text), opt(Type::Any)], Type::Table),
        ],
        schema_transform: Some(schema_select_columns),
        doc:              "Table.SelectColumns(prev, {col,...}, optional MissingField.X)",
    },
    FunctionDef {
        name:      "TransformColumnNames",
        arg_hints: vec![ArgKind::StepRef, ArgKind::EachExpr],
        signatures: vec![sig(vec![Type::Table, fun(vec![Type::Text], Type::Text)], Type::Table)],
        schema_transform: None, // column names unknown without evaluating the fn
        doc: "Table.TransformColumnNames(prev, each expr)",
    },
    fndef!("Unpivot", [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit, ArgKind::StringLit],
        sig(vec![Type::Table, list(Type::Text), Type::Text, Type::Text], Type::Table),
        "Table.Unpivot(prev, {col,...}, attrCol, valCol)"),
    fndef!("UnpivotOtherColumns", [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit, ArgKind::StringLit],
        sig(vec![Type::Table, list(Type::Text), Type::Text, Type::Text], Type::Table),
        "Table.UnpivotOtherColumns(prev, {col,...}, attrCol, valCol)"),

    // Transformation ──────────────────────────────────────────────────────
    FunctionDef {
        // (Table, Text, (Record) → T) → Table   — generic on column value type
        // Overload 2: with optional type annotation (4th arg)
        name:             "AddColumn",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::StringLit, ArgKind::EachExpr, ArgKind::OptValue],
        signatures: vec![
            sig(vec![Type::Table, Type::Text, fun(vec![Type::Record], tvar("T"))], Type::Table),
            sig(vec![Type::Table, Type::Text, fun(vec![Type::Record], tvar("T")), Type::Any], Type::Table),
        ],
        schema_transform: Some(schema_add_column),
        doc:              "Table.AddColumn(prev, name, each expr, optional type)",
    },
    fndef!("AddFuzzyClusterColumn",
        [ArgKind::StepRef, ArgKind::StringLit, ArgKind::StringLit, ArgKind::OptRecordLit],
        sig(vec![Type::Table, Type::Text, Type::Text], Type::Table),
        "Table.AddFuzzyClusterColumn(prev, col, newCol, optional options)"),
    FunctionDef {
        name:             "AddIndexColumn",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::StringLit, ArgKind::OptInteger, ArgKind::OptInteger],
        signatures: vec![
            sig(vec![Type::Table, Type::Text], Type::Table),
            sigp(vec![p(Type::Table), p(Type::Text), opt(Type::Number)], Type::Table),
            sigp(vec![p(Type::Table), p(Type::Text), opt(Type::Number), opt(Type::Number)], Type::Table),
        ],
        schema_transform: Some(schema_add_index_column),
        doc:              "Table.AddIndexColumn(prev, newCol, optional start, optional step)",
    },
    fndef!("AddJoinColumn",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit, ArgKind::OptJoinKind],
        sig(vec![Type::Table, list(Type::Text), Type::Table, list(Type::Text), Type::Text], Type::Table),
        "Table.AddJoinColumn(prev, {key}, other, {key}, newCol, optional JoinKind)"),
    fndef!("AddKey", [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::Value],
        sig(vec![Type::Table, list(Type::Text), Type::Any], Type::Table),
        "Table.AddKey(prev, {col,...}, isPrimary)"),
    fndef!("AggregateTableColumn", [ArgKind::StepRef, ArgKind::StringLit, ArgKind::AggregateList],
        sig(vec![Type::Table, Type::Text, list(Type::Any)], Type::Table),
        "Table.AggregateTableColumn(prev, col, {{name,each expr,type},...})"),
    FunctionDef {
        name:      "CombineColumns",
        arg_hints: vec![ArgKind::StepRef, ArgKind::ColumnList, ArgKind::EachExpr, ArgKind::StringLit],
        signatures: vec![sig(vec![Type::Table, list(Type::Text), fun(vec![list(Type::Any)], Type::Text), Type::Text], Type::Table)],
        schema_transform: None,
        doc: "Table.CombineColumns(prev, {col,...}, each combiner, newCol)",
    },
    fndef!("CombineColumnsToRecord", [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit],
        sig(vec![Type::Table, list(Type::Text), Type::Text], Type::Table),
        "Table.CombineColumnsToRecord(prev, {col,...}, newCol)"),
    fndef!("ExpandListColumn",   [ArgKind::StepRef, ArgKind::StringLit], sig(vec![Type::Table, Type::Text], Type::Table), "Table.ExpandListColumn(prev, col)"),
    fndef!("ExpandRecordColumn", [ArgKind::StepRef, ArgKind::StringLit, ArgKind::ColumnList],
        sig(vec![Type::Table, Type::Text, list(Type::Text)], Type::Table),
        "Table.ExpandRecordColumn(prev, col, {field,...})"),
    fndef!("ExpandTableColumn", [ArgKind::StepRef, ArgKind::StringLit, ArgKind::ColumnList],
        sig(vec![Type::Table, Type::Text, list(Type::Text)], Type::Table),
        "Table.ExpandTableColumn(prev, col, {col,...})"),
    FunctionDef {
        name:             "FillDown",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::ColumnList],
        signatures:       vec![sig(vec![Type::Table, list(Type::Text)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc:              "Table.FillDown(prev, {col,...})",
    },
    FunctionDef {
        name:             "FillUp",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::ColumnList],
        signatures:       vec![sig(vec![Type::Table, list(Type::Text)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc:              "Table.FillUp(prev, {col,...})",
    },
    fndef!("FuzzyGroup",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::AggregateList],
        sig(vec![Type::Table, list(Type::Text), list(Type::Any)], Type::Table),
        "Table.FuzzyGroup(prev, {key,...}, {{name,each expr,type},...})"),
    fndef!("FuzzyJoin",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StepRef, ArgKind::ColumnList, ArgKind::JoinKind],
        sig(vec![Type::Table, list(Type::Text), Type::Table, list(Type::Text), Type::Any], Type::Table),
        "Table.FuzzyJoin(prev, {key}, other, {key}, JoinKind.X)"),
    fndef!("FuzzyNestedJoin",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit, ArgKind::JoinKind],
        sig(vec![Type::Table, list(Type::Text), Type::Table, list(Type::Text), Type::Text, Type::Any], Type::Table),
        "Table.FuzzyNestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X)"),
    fndef!("Group",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::AggregateList],
        sig(vec![Type::Table, list(Type::Text), list(Type::Any)], Type::Table),
        "Table.Group(prev, {key,...}, {{name,each expr,type},...})"),
    fndef!("Join",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StepRef, ArgKind::ColumnList, ArgKind::JoinKind],
        sig(vec![Type::Table, list(Type::Text), Type::Table, list(Type::Text), Type::Any], Type::Table),
        "Table.Join(prev, {key}, other, {key}, JoinKind.X)"),
    fndef!("Keys", [ArgKind::StepRef], sig(vec![Type::Table], list(Type::Any)), "Table.Keys(prev)"),
    fndef!("NestedJoin",
        [ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StepRef, ArgKind::ColumnList, ArgKind::StringLit, ArgKind::JoinKind],
        sig(vec![Type::Table, list(Type::Text), Type::Table, list(Type::Text), Type::Text, Type::Any], Type::Table),
        "Table.NestedJoin(prev, {key}, other, {key}, newCol, JoinKind.X)"),
    fndef!("PartitionKey",      [ArgKind::StepRef], sig(vec![Type::Table], Type::Any), "Table.PartitionKey(prev)"),
    fndef!("ReplaceErrorValues", [ArgKind::StepRef, ArgKind::TransformList],
        sig(vec![Type::Table, list(Type::Any)], Type::Table),
        "Table.ReplaceErrorValues(prev, {{col,value},...})"),
    fndef!("ReplaceKeys",        [ArgKind::StepRef, ArgKind::ColumnList],  sig(vec![Type::Table, list(Type::Text)], Type::Table), "Table.ReplaceKeys(prev, {key,...})"),
    fndef!("ReplacePartitionKey",[ArgKind::StepRef, ArgKind::StringLit],  sig(vec![Type::Table, Type::Text], Type::Table),       "Table.ReplacePartitionKey(prev, key)"),
    FunctionDef {
        name:      "ReplaceValue",
        arg_hints: vec![ArgKind::StepRef, ArgKind::Value, ArgKind::Value, ArgKind::EachExpr],
        // (Table, T, T, (T, T) → Boolean) → Table
        signatures: vec![sig(vec![Type::Table, tvar("T"), tvar("T"), fun(vec![tvar("T"), tvar("T")], Type::Boolean)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc: "Table.ReplaceValue(prev, old, new, each replacer)",
    },
    FunctionDef {
        name:             "Sort",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::SortList],
        signatures:       vec![sig(vec![Type::Table, list(Type::Any)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc:              "Table.Sort(prev, {{col, Order.Ascending},...})",
    },
    fndef!("Split",      [ArgKind::StepRef, ArgKind::Integer], sig(vec![Type::Table, Type::Number], list(Type::Table)), "Table.Split(prev, pageSize)"),
    FunctionDef {
        name:      "SplitColumn",
        arg_hints: vec![ArgKind::StepRef, ArgKind::StringLit, ArgKind::EachExpr],
        signatures: vec![sig(vec![Type::Table, Type::Text, fun(vec![Type::Text], list(Type::Text))], Type::Table)],
        schema_transform: None,
        doc: "Table.SplitColumn(prev, col, each splitter)",
    },
    FunctionDef {
        name:             "TransformColumns",
        arg_hints:        vec![ArgKind::StepRefOrValue, ArgKind::TransformList, ArgKind::OptValue, ArgKind::OptMissingField],
        signatures:       vec![
            sig(vec![Type::Table, list(Type::Any)], Type::Table),
            sigp(vec![p(Type::Table), p(list(Type::Any)), opt(Type::Any), opt(Type::Number)], Type::Table),
        ],
        schema_transform: Some(schema_transform_columns),
        doc:              "Table.TransformColumns(prev, {{col,each expr},...}, optional defaultTransform, optional missingField)",
    },
    FunctionDef {
        name:             "TransformColumnTypes",
        arg_hints:        vec![ArgKind::StepRefOrValue, ArgKind::TypeList, ArgKind::OptCultureOrRecord],
        signatures:       vec![
            sig(vec![Type::Table, list(Type::Any)], Type::Table),
            sigp(vec![p(Type::Table), p(list(Type::Any)), opt(Type::Any)], Type::Table),
        ],
        schema_transform: Some(schema_transform_column_types),
        doc:              "Table.TransformColumnTypes(prev, {{col,type},...}, optional culture)",
    },
    FunctionDef {
        name:      "TransformRows",
        arg_hints: vec![ArgKind::StepRef, ArgKind::EachExpr],
        signatures: vec![sig(vec![Type::Table, fun(vec![Type::Record], Type::Record)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc: "Table.TransformRows(prev, each expr)",
    },
    fndef!("Transpose", [ArgKind::StepRef], sig(vec![Type::Table], Type::Table), "Table.Transpose(prev)"),

    // Membership ──────────────────────────────────────────────────────────
    fndef!("Contains",     [ArgKind::StepRef, ArgKind::RecordLit,  ArgKind::OptValue], sig(vec![Type::Table, Type::Record], Type::Boolean),       "Table.Contains(prev, [col=val,...], optional equationCriteria)"),
    fndef!("ContainsAll",  [ArgKind::StepRef, ArgKind::RecordList, ArgKind::OptValue], sig(vec![Type::Table, list(Type::Record)], Type::Boolean),  "Table.ContainsAll(prev, {[...],...}, optional equationCriteria)"),
    fndef!("ContainsAny",  [ArgKind::StepRef, ArgKind::RecordList],                   sig(vec![Type::Table, list(Type::Record)], Type::Boolean),  "Table.ContainsAny(prev, {[...],...})"),
    FunctionDef {
        name:             "Distinct",
        arg_hints:        vec![ArgKind::StepRef, ArgKind::ColumnList],
        signatures:       vec![sig(vec![Type::Table, list(Type::Text)], Type::Table)],
        schema_transform: Some(schema_passthrough),
        doc:              "Table.Distinct(prev, {col,...})",
    },
    fndef!("IsDistinct",    [ArgKind::StepRef],                    sig(vec![Type::Table], Type::Boolean),            "Table.IsDistinct(prev)"),
    fndef!("PositionOf",    [ArgKind::StepRef, ArgKind::RecordLit], sig(vec![Type::Table, Type::Record], Type::Number), "Table.PositionOf(prev, [col=val,...])"),
    fndef!("PositionOfAny", [ArgKind::StepRef, ArgKind::RecordList],sig(vec![Type::Table, list(Type::Record)], list(Type::Number)), "Table.PositionOfAny(prev, {[...],...})"),
    fndef!("RemoveMatchingRows",  [ArgKind::StepRef, ArgKind::RecordList],              sig(vec![Type::Table, list(Type::Record)], Type::Table),             "Table.RemoveMatchingRows(prev, {[...],...})"),
    fndef!("ReplaceMatchingRows", [ArgKind::StepRef, ArgKind::RecordList, ArgKind::RecordLit], sig(vec![Type::Table, list(Type::Record), Type::Record], Type::Table), "Table.ReplaceMatchingRows(prev, {[old]}, [new])"),

    // Ordering ────────────────────────────────────────────────────────────
    fndef!("AddRankColumn", [ArgKind::StepRef, ArgKind::StringLit, ArgKind::SortList, ArgKind::OptRecordLit],
        sig(vec![Type::Table, Type::Text, list(Type::Any)], Type::Table),
        "Table.AddRankColumn(prev, newCol, {{col,Order.X},...}, optional options)"),
    fndef!("Max",  [ArgKind::StepRef, ArgKind::StringLit, ArgKind::Value],   sig(vec![Type::Table, Type::Text, Type::Any], Type::Record), "Table.Max(prev, col, default)"),
    fndef!("MaxN", [ArgKind::StepRef, ArgKind::Integer,   ArgKind::StringLit],sig(vec![Type::Table, Type::Number, Type::Text], Type::Table),"Table.MaxN(prev, n, col)"),
    fndef!("Min",  [ArgKind::StepRef, ArgKind::StringLit, ArgKind::Value],   sig(vec![Type::Table, Type::Text, Type::Any], Type::Record), "Table.Min(prev, col, default)"),
    fndef!("MinN", [ArgKind::StepRef, ArgKind::Integer,   ArgKind::StringLit],sig(vec![Type::Table, Type::Number, Type::Text], Type::Table),"Table.MinN(prev, n, col)"),

    // Other ────────────────────────────────────────────────────────────────
    FunctionDef { name: "Buffer",              arg_hints: vec![ArgKind::StepRef], signatures: vec![sig(vec![Type::Table], Type::Table)], schema_transform: Some(schema_passthrough), doc: "Table.Buffer(prev)" },
    FunctionDef { name: "ConformToPageReader", arg_hints: vec![ArgKind::StepRef], signatures: vec![sig(vec![Type::Table], Type::Table)], schema_transform: Some(schema_passthrough), doc: "Table.ConformToPageReader(prev)" },
    FunctionDef { name: "StopFolding",         arg_hints: vec![ArgKind::StepRef], signatures: vec![sig(vec![Type::Table], Type::Table)], schema_transform: Some(schema_passthrough), doc: "Table.StopFolding(prev)" },
]}

// ── Tables namespace ──────────────────────────────────────────────────────

fn tables_functions() -> Vec<FunctionDef> { vec![
    FunctionDef {
        name:             "GetRelationships",
        arg_hints:        vec![ArgKind::ColumnList],
        signatures:       vec![sig(vec![list(Type::Table)], Type::Table)],
        schema_transform: None,
        doc:              "Tables.GetRelationships({table,...})",
    },
]}

// ── List namespace ────────────────────────────────────────────────────────

fn list_functions() -> Vec<FunctionDef> { vec![

    // ── Information ───────────────────────────────────────────────────────

    fndef!("Count",        [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], Type::Number),
        "List.Count(list) → number of items in list"),
    fndef!("IsEmpty",      [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], Type::Boolean),
        "List.IsEmpty(list) → true if list contains no items"),
    fndef!("NonNullCount", [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], Type::Number),
        "List.NonNullCount(list) → number of non-null items in list"),

    // ── Selection ─────────────────────────────────────────────────────────

    FunctionDef {
        name:      "Alternate",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer, ArgKind::Integer, ArgKind::OptInteger],
        signatures: vec![
            sig(vec![list(tvar("T")), Type::Number, Type::Number], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(Type::Number), p(Type::Number), opt(Type::Number)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Alternate(list, skip, take, optional offset) → odd-numbered offset elements",
    },
    FunctionDef {
        name:      "Buffer",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(tvar("T"))], list(tvar("T")))],
        schema_transform: None,
        doc: "List.Buffer(list) → buffers a list in memory",
    },
    FunctionDef {
        name:      "Distinct",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Distinct(list, optional equationCriteria) → list with duplicates removed",
    },
    fndef!("FindText", [ArgKind::StepRefOrValue, ArgKind::StringLit],
        sig(vec![list(Type::Any), Type::Text], list(Type::Any)),
        "List.FindText(list, text) → values (including record fields) containing text"),
    FunctionDef {
        name:      "First",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(tvar("T"))], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.First(list, optional default) → first value or default if empty",
    },
    FunctionDef {
        name:      "FirstN",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value],
        signatures: vec![sig(vec![list(tvar("T")), Type::Any], list(tvar("T")))],
        schema_transform: None,
        doc: "List.FirstN(list, countOrCondition) → first N items or items matching condition",
    },
    FunctionDef {
        name:      "InsertRange",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer, ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(tvar("T")), Type::Number, list(tvar("T"))], list(tvar("T")))],
        schema_transform: None,
        doc: "List.InsertRange(list, index, values) → list with values inserted at index",
    },
    FunctionDef {
        name:      "IsDistinct",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(Type::Any)], Type::Boolean),
            sigp(vec![p(list(Type::Any)), opt(Type::Any)], Type::Boolean),
        ],
        schema_transform: None,
        doc: "List.IsDistinct(list, optional equationCriteria) → true if no duplicates in list",
    },
    FunctionDef {
        name:      "Last",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(tvar("T"))], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.Last(list, optional default) → last value or default if empty",
    },
    FunctionDef {
        name:      "LastN",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value],
        signatures: vec![sig(vec![list(tvar("T")), Type::Any], list(tvar("T")))],
        schema_transform: None,
        doc: "List.LastN(list, countOrCondition) → last N items or items matching condition",
    },
    FunctionDef {
        name:      "MatchesAll",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::EachExpr],
        signatures: vec![sig(vec![list(tvar("T")), fun(vec![tvar("T")], Type::Boolean)], Type::Boolean)],
        schema_transform: None,
        doc: "List.MatchesAll(list, condition) → true if all values satisfy condition",
    },
    FunctionDef {
        name:      "MatchesAny",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::EachExpr],
        signatures: vec![sig(vec![list(tvar("T")), fun(vec![tvar("T")], Type::Boolean)], Type::Boolean)],
        schema_transform: None,
        doc: "List.MatchesAny(list, condition) → true if any value satisfies condition",
    },
    fndef!("Positions", [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], list(Type::Number)),
        "List.Positions(list) → list of offsets for the input list"),
    FunctionDef {
        name:      "Range",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer, ArgKind::OptInteger],
        signatures: vec![
            sig(vec![list(tvar("T")), Type::Number], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(Type::Number), opt(nullable(Type::Number))], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Range(list, offset, optional count) → subset of list beginning at offset",
    },
    FunctionDef {
        name:      "Select",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::EachExpr],
        signatures: vec![sig(vec![list(tvar("T")), fun(vec![tvar("T")], Type::Boolean)], list(tvar("T")))],
        schema_transform: None,
        doc: "List.Select(list, each predicate) → values matching condition",
    },
    FunctionDef {
        name:      "Single",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(tvar("T"))], tvar("T"))],
        schema_transform: None,
        doc: "List.Single(list) → the one item in a single-element list; error otherwise",
    },
    FunctionDef {
        name:      "SingleOrDefault",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(tvar("T"))], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.SingleOrDefault(list, optional default) → single item or default for empty list",
    },
    FunctionDef {
        name:      "Skip",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value],
        signatures: vec![sig(vec![list(tvar("T")), Type::Any], list(tvar("T")))],
        schema_transform: None,
        doc: "List.Skip(list, countOrCondition) → list with leading items removed",
    },

    // ── Transformation ────────────────────────────────────────────────────

    FunctionDef {
        name:      "Accumulate",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::EachExpr],
        // (List<T>, U, (U, T) → U) → U
        signatures: vec![sig(
            vec![list(tvar("T")), tvar("U"), fun(vec![tvar("U"), tvar("T")], tvar("U"))],
            tvar("U"),
        )],
        schema_transform: None,
        doc: "List.Accumulate(list, seed, (acc, x) => expr) → accumulated summary value",
    },
    FunctionDef {
        name:      "Combine",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(list(tvar("T")))], list(tvar("T")))],
        schema_transform: None,
        doc: "List.Combine({list1, list2, ...}) → single combined list",
    },
    FunctionDef {
        name:      "RemoveFirstN",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value],
        signatures: vec![sig(vec![list(tvar("T")), Type::Any], list(tvar("T")))],
        schema_transform: None,
        doc: "List.RemoveFirstN(list, countOrCondition) → list with first N elements removed",
    },
    FunctionDef {
        name:      "RemoveItems",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(tvar("T")), list(tvar("T"))], list(tvar("T")))],
        schema_transform: None,
        doc: "List.RemoveItems(list1, list2) → list1 minus items present in list2",
    },
    FunctionDef {
        name:      "RemoveLastN",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value],
        signatures: vec![sig(vec![list(tvar("T")), Type::Any], list(tvar("T")))],
        schema_transform: None,
        doc: "List.RemoveLastN(list, countOrCondition) → list with last N elements removed",
    },
    FunctionDef {
        name:      "RemoveMatchingItems",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), list(tvar("T"))], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(list(tvar("T"))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.RemoveMatchingItems(list, values, optional equationCriteria) → all occurrences removed",
    },
    FunctionDef {
        name:      "RemoveNulls",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(nullable(tvar("T")))], list(tvar("T")))],
        schema_transform: None,
        doc: "List.RemoveNulls(list) → list with all null values removed",
    },
    FunctionDef {
        name:      "RemoveRange",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer, ArgKind::OptInteger],
        signatures: vec![
            sig(vec![list(tvar("T")), Type::Number], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(Type::Number), opt(Type::Number)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.RemoveRange(list, index, optional count) → list with range of values removed",
    },
    FunctionDef {
        name:      "Repeat",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer],
        signatures: vec![sig(vec![list(tvar("T")), Type::Number], list(tvar("T")))],
        schema_transform: None,
        doc: "List.Repeat(list, count) → list repeated count times",
    },
    FunctionDef {
        name:      "ReplaceMatchingItems",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), list(list(tvar("T")))], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(list(list(tvar("T")))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.ReplaceMatchingItems(list, replacements, optional equationCriteria) → replacements applied",
    },
    FunctionDef {
        name:      "ReplaceRange",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer, ArgKind::Integer, ArgKind::StepRefOrValue],
        signatures: vec![sig(
            vec![list(tvar("T")), Type::Number, Type::Number, list(tvar("T"))],
            list(tvar("T")),
        )],
        schema_transform: None,
        doc: "List.ReplaceRange(list, index, count, replaceWith) → list with range replaced",
    },
    FunctionDef {
        name:      "ReplaceValue",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::Value, ArgKind::EachExpr],
        signatures: vec![sig(
            vec![list(tvar("T")), tvar("T"), tvar("T"), fun(vec![tvar("T"), tvar("T")], Type::Boolean)],
            list(tvar("T")),
        )],
        schema_transform: None,
        doc: "List.ReplaceValue(list, oldValue, newValue, replacer) → list with value replaced",
    },
    FunctionDef {
        name:      "Reverse",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(tvar("T"))], list(tvar("T")))],
        schema_transform: None,
        doc: "List.Reverse(list) → list in reversed order",
    },
    FunctionDef {
        name:      "Split",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Integer],
        signatures: vec![sig(vec![list(tvar("T")), Type::Number], list(list(tvar("T"))))],
        schema_transform: None,
        doc: "List.Split(list, pageSize) → list of sub-lists each of length pageSize",
    },
    FunctionDef {
        name:      "Transform",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::EachExpr],
        // (List<T>, (T) → U) → List<U>
        signatures: vec![sig(vec![list(tvar("T")), fun(vec![tvar("T")], tvar("U"))], list(tvar("U")))],
        schema_transform: None,
        doc: "List.Transform(list, each expr) → new list of transformed values",
    },
    FunctionDef {
        name:      "TransformMany",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::EachExpr, ArgKind::EachExpr],
        // (List<T>, (T) → List<U>, (T, U) → R) → List<R>
        signatures: vec![sig(
            vec![
                list(tvar("T")),
                fun(vec![tvar("T")], list(tvar("U"))),
                fun(vec![tvar("T"), tvar("U")], tvar("R")),
            ],
            list(tvar("R")),
        )],
        schema_transform: None,
        doc: "List.TransformMany(list, listTransform, resultTransform) → flattened transformed list",
    },
    FunctionDef {
        name:      "Zip",
        arg_hints: vec![ArgKind::StepRefOrValue],
        signatures: vec![sig(vec![list(list(Type::Any))], list(list(Type::Any)))],
        schema_transform: None,
        doc: "List.Zip({list1, list2, ...}) → list of lists combining items at same position",
    },

    // ── Membership ────────────────────────────────────────────────────────

    fndef!("AllTrue",  [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Boolean)], Type::Boolean),
        "List.AllTrue(list) → true if all expressions are true"),
    fndef!("AnyTrue",  [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Boolean)], Type::Boolean),
        "List.AnyTrue(list) → true if any expression is true"),
    FunctionDef {
        name:      "Contains",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), tvar("T")], Type::Boolean),
            sigp(vec![p(list(tvar("T"))), p(tvar("T")), opt(Type::Any)], Type::Boolean),
        ],
        schema_transform: None,
        doc: "List.Contains(list, value, optional equationCriteria) → true if list contains value",
    },
    FunctionDef {
        name:      "ContainsAll",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), list(tvar("T"))], Type::Boolean),
            sigp(vec![p(list(tvar("T"))), p(list(tvar("T"))), opt(Type::Any)], Type::Boolean),
        ],
        schema_transform: None,
        doc: "List.ContainsAll(list, values, optional equationCriteria) → true if list includes all values",
    },
    FunctionDef {
        name:      "ContainsAny",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), list(tvar("T"))], Type::Boolean),
            sigp(vec![p(list(tvar("T"))), p(list(tvar("T"))), opt(Type::Any)], Type::Boolean),
        ],
        schema_transform: None,
        doc: "List.ContainsAny(list, values, optional equationCriteria) → true if list includes any value",
    },
    FunctionDef {
        name:      "PositionOf",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::OptValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), tvar("T")], Type::Any),
            sigp(vec![p(list(tvar("T"))), p(tvar("T")), opt(Type::Number), opt(Type::Any)], Type::Any),
        ],
        schema_transform: None,
        doc: "List.PositionOf(list, value, optional occurrence, optional equationCriteria) → offset(s)",
    },
    FunctionDef {
        name:      "PositionOfAny",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue, ArgKind::OptValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), list(tvar("T"))], Type::Number),
            sigp(vec![p(list(tvar("T"))), p(list(tvar("T"))), opt(Type::Number), opt(Type::Any)], Type::Number),
        ],
        schema_transform: None,
        doc: "List.PositionOfAny(list, values, optional occurrence, optional equationCriteria) → first offset",
    },

    // ── Set operations ────────────────────────────────────────────────────

    FunctionDef {
        name:      "Difference",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), list(tvar("T"))], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(list(tvar("T"))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Difference(list1, list2, optional equationCriteria) → items in list1 not in list2",
    },
    FunctionDef {
        name:      "Intersect",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(list(tvar("T")))], list(tvar("T"))),
            sigp(vec![p(list(list(tvar("T")))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Intersect(lists, optional equationCriteria) → intersection of list values",
    },
    FunctionDef {
        name:      "Union",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(list(tvar("T")))], list(tvar("T"))),
            sigp(vec![p(list(list(tvar("T")))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Union(lists, optional equationCriteria) → union of list values",
    },

    // ── Ordering ──────────────────────────────────────────────────────────

    FunctionDef {
        name:      "Max",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(tvar("T"))], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.Max(list, optional default) → maximum value or default for empty list",
    },
    FunctionDef {
        name:      "MaxN",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), Type::Any], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(Type::Any), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.MaxN(list, countOrCondition, optional comparisonCriteria) → maximum N values",
    },
    FunctionDef {
        name:      "Median",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(Type::Any)], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.Median(list, optional comparisonCriteria) → median value in list",
    },
    FunctionDef {
        name:      "Min",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(tvar("T"))], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.Min(list, optional default) → minimum value or default for empty list",
    },
    FunctionDef {
        name:      "MinN",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T")), Type::Any], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), p(Type::Any), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.MinN(list, countOrCondition, optional comparisonCriteria) → minimum N values",
    },
    FunctionDef {
        name:      "Percentile",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::Value, ArgKind::OptRecordLit],
        signatures: vec![
            sig(vec![list(tvar("T")), Type::Any], Type::Any),
            sigp(vec![p(list(tvar("T"))), p(Type::Any), opt(Type::Record)], Type::Any),
        ],
        schema_transform: None,
        doc: "List.Percentile(list, percentiles, optional options) → sample percentile(s)",
    },
    FunctionDef {
        name:      "Sort",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Sort(list, optional comparisonCriteria) → sorted list",
    },

    // ── Averages ──────────────────────────────────────────────────────────

    fndef!("Average", [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], Type::Any),
        "List.Average(list) → average of number/date/datetime/datetimezone/duration values"),
    FunctionDef {
        name:      "Mode",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], tvar("T")),
            sigp(vec![p(list(tvar("T"))), opt(Type::Any)], tvar("T")),
        ],
        schema_transform: None,
        doc: "List.Mode(list, optional equationCriteria) → most frequently occurring value",
    },
    FunctionDef {
        name:      "Modes",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(tvar("T"))], list(tvar("T"))),
            sigp(vec![p(list(tvar("T"))), opt(Type::Any)], list(tvar("T"))),
        ],
        schema_transform: None,
        doc: "List.Modes(list, optional equationCriteria) → list of most frequently occurring values",
    },
    fndef!("StandardDeviation", [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], Type::Any),
        "List.StandardDeviation(list) → sample-based standard deviation estimate"),

    // ── Addition ──────────────────────────────────────────────────────────

    fndef!("Sum", [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Any)], Type::Any),
        "List.Sum(list) → sum of number or duration items"),

    // ── Numerics ──────────────────────────────────────────────────────────

    fndef!("Covariance", [ArgKind::StepRefOrValue, ArgKind::StepRefOrValue],
        sig(vec![list(Type::Number), list(Type::Number)], Type::Number),
        "List.Covariance(list1, list2) → covariance between two number lists"),
    fndef!("Product", [ArgKind::StepRefOrValue],
        sig(vec![list(Type::Number)], Type::Number),
        "List.Product(list) → product of numbers in list"),

    // ── Generators ────────────────────────────────────────────────────────

    fndef!("Dates",         [ArgKind::Value, ArgKind::Integer, ArgKind::Value],
        sig(vec![Type::Any, Type::Number, Type::Any], list(Type::Any)),
        "List.Dates(start, count, step) → list of date values"),
    fndef!("DateTimes",     [ArgKind::Value, ArgKind::Integer, ArgKind::Value],
        sig(vec![Type::Any, Type::Number, Type::Any], list(Type::Any)),
        "List.DateTimes(start, count, step) → list of datetime values"),
    fndef!("DateTimeZones", [ArgKind::Value, ArgKind::Integer, ArgKind::Value],
        sig(vec![Type::Any, Type::Number, Type::Any], list(Type::Any)),
        "List.DateTimeZones(start, count, step) → list of datetimezone values"),
    fndef!("Durations",     [ArgKind::Value, ArgKind::Integer, ArgKind::Value],
        sig(vec![Type::Any, Type::Number, Type::Any], list(Type::Any)),
        "List.Durations(start, count, step) → list of duration values"),
    FunctionDef {
        name:      "Generate",
        arg_hints: vec![ArgKind::Value, ArgKind::EachExpr, ArgKind::EachExpr, ArgKind::OptValue],
        signatures: vec![
            sig(
                vec![tvar("T"), fun(vec![tvar("T")], Type::Boolean), fun(vec![tvar("T")], tvar("T"))],
                list(tvar("T")),
            ),
            sigp(vec![
                p(tvar("T")),
                p(fun(vec![tvar("T")], Type::Boolean)),
                p(fun(vec![tvar("T")], tvar("T"))),
                opt(fun(vec![tvar("T")], tvar("U"))),
            ], list(tvar("U"))),
        ],
        schema_transform: None,
        doc: "List.Generate(initial, condition, next, optional selector) → generated list",
    },
    FunctionDef {
        name:      "Numbers",
        arg_hints: vec![ArgKind::Value, ArgKind::Integer, ArgKind::OptValue],
        signatures: vec![
            sig(vec![Type::Number, Type::Number], list(Type::Number)),
            sigp(vec![p(Type::Number), p(Type::Number), opt(Type::Number)], list(Type::Number)),
        ],
        schema_transform: None,
        doc: "List.Numbers(start, count, optional increment) → list of numbers",
    },
    FunctionDef {
        name:      "Random",
        arg_hints: vec![ArgKind::Integer, ArgKind::OptValue],
        signatures: vec![
            sig(vec![Type::Number], list(Type::Number)),
            sigp(vec![p(Type::Number), opt(Type::Number)], list(Type::Number)),
        ],
        schema_transform: None,
        doc: "List.Random(count, optional seed) → list of random numbers between 0 and 1",
    },
    fndef!("Times", [ArgKind::Value, ArgKind::Integer, ArgKind::Value],
        sig(vec![Type::Any, Type::Number, Type::Any], list(Type::Any)),
        "List.Times(start, count, step) → list of time values"),
]}

// ── Text namespace ────────────────────────────────────────────────────────

fn text_functions() -> Vec<FunctionDef> { vec![
    fndef!("Length",     [ArgKind::Value],  sig(vec![Type::Text], Type::Number),   "Text.Length(text)"),
    fndef!("From",       [ArgKind::Value],  sig(vec![Type::Any], Type::Text),      "Text.From(value)"),
    fndef!("Upper",      [ArgKind::Value],  sig(vec![Type::Text], Type::Text),     "Text.Upper(text)"),
    fndef!("Lower",      [ArgKind::Value],  sig(vec![Type::Text], Type::Text),     "Text.Lower(text)"),
    fndef!("Trim",       [ArgKind::Value],  sig(vec![Type::Text], Type::Text),     "Text.Trim(text)"),
    fndef!("TrimStart",  [ArgKind::Value],  sig(vec![Type::Text], Type::Text),     "Text.TrimStart(text)"),
    fndef!("TrimEnd",    [ArgKind::Value],  sig(vec![Type::Text], Type::Text),     "Text.TrimEnd(text)"),
    fndef!("PadStart",   [ArgKind::Value, ArgKind::Integer, ArgKind::StringLit],
        sig(vec![Type::Text, Type::Number, Type::Text], Type::Text), "Text.PadStart(text,width,pad)"),
    fndef!("PadEnd",     [ArgKind::Value, ArgKind::Integer, ArgKind::StringLit],
        sig(vec![Type::Text, Type::Number, Type::Text], Type::Text), "Text.PadEnd(text,width,pad)"),
    FunctionDef {
        name:             "Contains",
        arg_hints:        vec![ArgKind::Value, ArgKind::Value, ArgKind::OptValue],
        signatures:       vec![
            sig(vec![nullable(Type::Text), Type::Text], nullable(Type::Boolean)),
            sigp(vec![p(nullable(Type::Text)), p(Type::Text), opt(nullable(Type::Any))], nullable(Type::Boolean)),
        ],
        schema_transform: None,
        doc:              "Text.Contains(text, substring, optional comparer)",
    },
    FunctionDef {
        name:             "StartsWith",
        arg_hints:        vec![ArgKind::Value, ArgKind::Value, ArgKind::OptValue],
        signatures:       vec![
            sig(vec![nullable(Type::Text), Type::Text], nullable(Type::Boolean)),
            sigp(vec![p(nullable(Type::Text)), p(Type::Text), opt(nullable(Type::Any))], nullable(Type::Boolean)),
        ],
        schema_transform: None,
        doc:              "Text.StartsWith(text, substring, optional comparer)",
    },
    FunctionDef {
        name:             "EndsWith",
        arg_hints:        vec![ArgKind::Value, ArgKind::Value, ArgKind::OptValue],
        signatures:       vec![
            sig(vec![nullable(Type::Text), Type::Text], nullable(Type::Boolean)),
            sigp(vec![p(nullable(Type::Text)), p(Type::Text), opt(nullable(Type::Any))], nullable(Type::Boolean)),
        ],
        schema_transform: None,
        doc:              "Text.EndsWith(text, substring, optional comparer)",
    },
    fndef!("Start",      [ArgKind::Value, ArgKind::Integer],
        sig(vec![Type::Text, Type::Number], Type::Text), "Text.Start(text, count)"),
    fndef!("End",        [ArgKind::Value, ArgKind::Integer],
        sig(vec![Type::Text, Type::Number], Type::Text), "Text.End(text, count)"),
    fndef!("Range",      [ArgKind::Value, ArgKind::Integer, ArgKind::Integer],
        sig(vec![Type::Text, Type::Number, Type::Number], Type::Text), "Text.Range(text,offset,count)"),
    fndef!("Replace",    [ArgKind::Value, ArgKind::StringLit, ArgKind::StringLit],
        sig(vec![Type::Text, Type::Text, Type::Text], Type::Text), "Text.Replace(text,old,new)"),
    fndef!("Split",      [ArgKind::Value, ArgKind::StringLit],
        sig(vec![Type::Text, Type::Text], list(Type::Text)), "Text.Split(text,delimiter)"),
    // Two overloads: with and without separator
    FunctionDef {
        name:      "Combine",
        arg_hints: vec![ArgKind::StepRefOrValue, ArgKind::OptValue],
        signatures: vec![
            sig(vec![list(Type::Text)], Type::Text),
            sig(vec![list(Type::Text), Type::Text], Type::Text),
        ],
        schema_transform: None,
        doc: "Text.Combine(list) or Text.Combine(list, separator)",
    },
]}

// ── Number namespace ──────────────────────────────────────────────────────

fn number_functions() -> Vec<FunctionDef> { vec![
    fndef!("From",      [ArgKind::Value],              sig(vec![Type::Any], Type::Number),           "Number.From(value)"),
    // Three overloads: Round(n), Round(n,digits), Round(n,digits,mode)
    FunctionDef {
        name:      "Round",
        arg_hints: vec![ArgKind::Value, ArgKind::OptInteger, ArgKind::OptInteger],
        signatures: vec![
            sig(vec![Type::Number], Type::Number),
            sig(vec![Type::Number, Type::Number], Type::Number),
            sig(vec![Type::Number, Type::Number, Type::Number], Type::Number),
        ],
        schema_transform: None,
        doc: "Number.Round(n), Number.Round(n, digits), Number.Round(n, digits, mode)",
    },
    fndef!("RoundUp",   [ArgKind::Value], sig(vec![Type::Number], Type::Number), "Number.RoundUp(n)"),
    fndef!("RoundDown", [ArgKind::Value], sig(vec![Type::Number], Type::Number), "Number.RoundDown(n)"),
    fndef!("Abs",       [ArgKind::Value], sig(vec![Type::Number], Type::Number), "Number.Abs(n)"),
    fndef!("Sqrt",      [ArgKind::Value], sig(vec![Type::Number], Type::Number), "Number.Sqrt(n)"),
    fndef!("Power",     [ArgKind::Value, ArgKind::Value], sig(vec![Type::Number, Type::Number], Type::Number), "Number.Power(base,exponent)"),
    fndef!("Log",       [ArgKind::Value], sig(vec![Type::Number], Type::Number), "Number.Log(n)"),
    fndef!("Mod",       [ArgKind::Value, ArgKind::Value], sig(vec![Type::Number, Type::Number], Type::Number), "Number.Mod(n,divisor)"),
    fndef!("Sign",      [ArgKind::Value], sig(vec![Type::Number], Type::Number), "Number.Sign(n)"),
]}

// ── Logical namespace ─────────────────────────────────────────────────────

fn logical_functions() -> Vec<FunctionDef> { vec![
    fndef!("From", [ArgKind::Value],                   sig(vec![Type::Any], Type::Boolean),              "Logical.From(value)"),
    fndef!("Not",  [ArgKind::Value],                   sig(vec![Type::Boolean], Type::Boolean),          "Logical.Not(b)"),
    fndef!("And",  [ArgKind::Value, ArgKind::Value],   sig(vec![Type::Boolean, Type::Boolean], Type::Boolean), "Logical.And(a,b)"),
    fndef!("Or",   [ArgKind::Value, ArgKind::Value],   sig(vec![Type::Boolean, Type::Boolean], Type::Boolean), "Logical.Or(a,b)"),
    fndef!("Xor",  [ArgKind::Value, ArgKind::Value],   sig(vec![Type::Boolean, Type::Boolean], Type::Boolean), "Logical.Xor(a,b)"),
]}

// ── Lookup helpers ────────────────────────────────────────────────────────

pub fn lookup_namespace(name: &str) -> Option<&'static NamespaceDef> {
    registry().iter().find(|ns| ns.name == name)
}

pub fn lookup_function(namespace: &str, function: &str) -> Option<&'static FunctionDef> {
    lookup_namespace(namespace)?.functions.iter().find(|f| f.name == function)
}

pub fn lookup_qualified(qualified: &str) -> Option<&'static FunctionDef> {
    let (ns, func) = qualified.split_once('.')?;
    lookup_function(ns, func)
}

pub fn functions_in_namespace(namespace: &str) -> Vec<&'static str> {
    lookup_namespace(namespace)
        .map(|ns| ns.functions.iter().map(|f| f.name).collect())
        .unwrap_or_default()
}

pub fn all_qualified_names() -> Vec<String> {
    registry()
        .iter()
        .flat_map(|ns| ns.functions.iter().map(move |f| format!("{}.{}", ns.name, f.name)))
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Type, unify, fun, list, tvar, nullable};
    use std::collections::HashMap;

    #[test]
    fn test_lookup_namespace() {
        assert!(lookup_namespace("Table").is_some());
        assert!(lookup_namespace("Excel").is_some());
        assert!(lookup_namespace("List").is_some());
        assert!(lookup_namespace("Text").is_some());
        assert!(lookup_namespace("Number").is_some());
        assert!(lookup_namespace("Logical").is_some());
        assert!(lookup_namespace("Unknown").is_none());
    }

    #[test]
    fn test_lookup_function_arg_hints() {
        let f = lookup_function("Table", "SelectRows").unwrap();
        assert_eq!(f.name, "SelectRows");
        assert_eq!(f.arg_hints.len(), 2);
        assert_eq!(f.arg_hints[0], ArgKind::StepRef);
        assert_eq!(f.arg_hints[1], ArgKind::EachExpr);
    }

    #[test]
    fn test_select_rows_signature() {
        let f   = lookup_function("Table", "SelectRows").unwrap();
        let sig = f.primary_sig();
        // (Table, (Record) → Boolean) → Table
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0].ty, Type::Table);
        assert_eq!(sig.params[1].ty, fun(vec![Type::Record], Type::Boolean));
        assert!(!sig.params[0].optional);
        assert!(!sig.params[1].optional);
        assert_eq!(*sig.return_type, Type::Table);
        assert_eq!(sig.to_string(), "(Table, (Record) → Boolean) → Table");
    }

    #[test]
    fn test_select_rows_has_schema_passthrough() {
        let f = lookup_function("Table", "SelectRows").unwrap();
        assert!(f.schema_transform.is_some());
        // passthrough: output schema = input schema
        let args = SchemaTransformArgs {
            input_schema: Some(
                TableSchema::default()
                    .with_column("Name", Type::Text)
                    .with_column("Age",  Type::Number)
            ),
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.columns.len(), 2);
    }

    #[test]
    fn test_add_column_signature_and_overloads() {
        let f = lookup_function("Table", "AddColumn").unwrap();
        assert_eq!(f.signatures.len(), 2);   // base + with-type overload
        let s0 = &f.signatures[0];
        assert_eq!(s0.params[0].ty, Type::Table);
        assert_eq!(s0.params[1].ty, Type::Text);
        assert_eq!(s0.params[2].ty, fun(vec![Type::Record], tvar("T")));
        // overload for 3 args picks first signature
        assert!(f.overload_for_arity(3).is_some());
        // overload for 4 args picks second signature
        assert!(f.overload_for_arity(4).is_some());
    }

    #[test]
    fn test_add_column_schema_transform() {
        let f = lookup_function("Table", "AddColumn").unwrap();
        assert!(f.schema_transform.is_some());
        let args = SchemaTransformArgs {
            input_schema: Some(TableSchema::default().with_column("Name", Type::Text)),
            str_args:     vec!["Score".to_string()],
            fn_return_type: Some(Type::Number),
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.columns.len(), 2);
        assert_eq!(out.get_column("Score").unwrap().ty, Type::Number);
    }

    #[test]
    fn test_remove_columns_schema_transform() {
        let f = lookup_function("Table", "RemoveColumns").unwrap();
        let args = SchemaTransformArgs {
            input_schema: Some(
                TableSchema::default()
                    .with_column("A", Type::Text)
                    .with_column("B", Type::Number)
                    .with_column("C", Type::Boolean)
            ),
            str_args: vec!["B".to_string()],
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.columns.len(), 2);
        assert!(out.get_column("B").is_none());
    }

    #[test]
    fn test_rename_columns_schema_transform() {
        let f = lookup_function("Table", "RenameColumns").unwrap();
        let args = SchemaTransformArgs {
            input_schema: Some(TableSchema::default().with_column("OldName", Type::Text)),
            rename_pairs: vec![("OldName".to_string(), "NewName".to_string())],
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert!(out.get_column("OldName").is_none());
        assert_eq!(out.get_column("NewName").unwrap().ty, Type::Text);
    }

    #[test]
    fn test_select_columns_schema_transform() {
        let f = lookup_function("Table", "SelectColumns").unwrap();
        let args = SchemaTransformArgs {
            input_schema: Some(
                TableSchema::default()
                    .with_column("A", Type::Text)
                    .with_column("B", Type::Number)
                    .with_column("C", Type::Boolean)
            ),
            str_args: vec!["C".to_string(), "A".to_string()],
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.columns.len(), 2);
        assert_eq!(out.columns[0].name, "C");
        assert_eq!(out.columns[1].name, "A");
    }

    #[test]
    fn test_transform_column_types_schema_transform() {
        let f = lookup_function("Table", "TransformColumnTypes").unwrap();
        let args = SchemaTransformArgs {
            input_schema: Some(TableSchema::default().with_column("Age", Type::Text)),
            type_pairs:   vec![("Age".to_string(), Type::Number)],
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.get_column("Age").unwrap().ty, Type::Number);
    }

    #[test]
    fn test_add_index_column_schema_transform() {
        let f = lookup_function("Table", "AddIndexColumn").unwrap();
        assert_eq!(f.signatures.len(), 3); // 3 overloads
        assert!(f.overload_for_arity(2).is_some());
        assert!(f.overload_for_arity(3).is_some());
        assert!(f.overload_for_arity(4).is_some());

        let args = SchemaTransformArgs {
            input_schema: Some(TableSchema::default().with_column("Name", Type::Text)),
            str_args:     vec!["Index".to_string()],
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.columns.len(), 2);
        assert_eq!(out.get_column("Index").unwrap().ty, Type::Number);
    }

    #[test]
    fn test_number_round_overloads() {
        let f = lookup_function("Number", "Round").unwrap();
        assert_eq!(f.signatures.len(), 3);
        assert!(f.overload_for_arity(1).is_some()); // Round(n)
        assert!(f.overload_for_arity(2).is_some()); // Round(n, digits)
        assert!(f.overload_for_arity(3).is_some()); // Round(n, digits, mode)
        assert!(f.overload_for_arity(0).is_none());
        assert!(f.overload_for_arity(4).is_none());
    }

    #[test]
    fn test_text_combine_overloads() {
        let f = lookup_function("Text", "Combine").unwrap();
        assert_eq!(f.signatures.len(), 2);
        assert!(f.overload_for_arity(1).is_some()); // no separator
        assert!(f.overload_for_arity(2).is_some()); // with separator
    }

    #[test]
    fn test_list_transform_signature() {
        let f   = lookup_function("List", "Transform").unwrap();
        let sig = f.primary_sig();
        assert_eq!(sig.params[0].ty, list(tvar("T")));
        assert_eq!(sig.params[1].ty, fun(vec![tvar("T")], tvar("U")));
        assert_eq!(*sig.return_type,  list(tvar("U")));
    }

    #[test]
    fn test_list_transform_generic_instantiation() {
        let f   = lookup_function("List", "Transform").unwrap();
        let sig = f.primary_sig();
        let mut subst: HashMap<String, Type> = HashMap::new();
        unify(&sig.params[0].ty, &list(Type::Number), &mut subst);
        unify(&sig.params[1].ty, &fun(vec![Type::Number], Type::Text), &mut subst);
        let ret = sig.return_type.substitute(&subst);
        assert_eq!(ret, list(Type::Text));
    }

    #[test]
    fn test_nullable_in_schema() {
        // Column schemas can hold nullable types
        let schema = TableSchema::default()
            .with_column("Name",   nullable(Type::Text))
            .with_column("Amount", nullable(Type::Number));
        assert_eq!(schema.get_column("Name").unwrap().ty, nullable(Type::Text));
    }

    #[test]
    fn test_optional_param_signature() {
        // AddIndexColumn uses sigp() with optional params
        let f = lookup_function("Table", "AddIndexColumn").unwrap();
        let sig3 = &f.signatures[2]; // 3-param overload with optional params
        assert!(!sig3.params[0].optional);
        assert!(!sig3.params[1].optional);
        assert!( sig3.params[2].optional); // start is optional
    }

    #[test]
    fn test_primary_sig() {
        let f = lookup_function("Number", "Round").unwrap();
        assert_eq!(f.primary_sig().required_arity(), 1);
        assert_eq!(f.primary_sig().max_arity(), 1);
    }

    #[test]
    fn test_lookup_qualified() {
        assert_eq!(lookup_qualified("Excel.Workbook").unwrap().name, "Workbook");
        assert_eq!(lookup_qualified("List.Transform").unwrap().name, "Transform");
        assert_eq!(lookup_qualified("Text.Length").unwrap().name, "Length");
    }

    #[test]
    fn test_all_qualified_names_coverage() {
        let names = all_qualified_names();
        for n in &["Table.SelectRows","Excel.Workbook","List.Transform","Text.Length","Number.From","Logical.From"] {
            assert!(names.contains(&n.to_string()), "missing {}", n);
        }
    }

    #[test]
    fn test_functions_in_namespace() {
        let t = functions_in_namespace("Table");
        assert!(t.contains(&"SelectRows") && t.contains(&"AddColumn") && !t.contains(&"Workbook"));
    }

    #[test]
    fn test_duplicate_column_schema_transform() {
        let f = lookup_function("Table", "DuplicateColumn").unwrap();
        let args = SchemaTransformArgs {
            input_schema: Some(TableSchema::default().with_column("Price", Type::Number)),
            str_args:     vec!["Price".to_string(), "PriceCopy".to_string()],
            ..Default::default()
        };
        let out = (f.schema_transform.unwrap())(&args).unwrap();
        assert_eq!(out.columns.len(), 2);
        assert_eq!(out.get_column("PriceCopy").unwrap().ty, Type::Number);
    }
}
