use pq_diagnostics::Span;
use pq_types::ColumnType;
use crate::ExprNode;
use crate::call_arg::CallArg;

/// How `Table.RemoveColumns` (and other column-selector functions) should
/// behave when a named column is absent from the input table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingFieldKind {
    Error,
    Ignore,
    UseNull,
}

/// A single named transformation step.  Every step carries a Span pointing to
/// where it appears in the source formula.
#[derive(Debug, Clone)]
pub struct Step {
    pub kind:        StepKind,
    pub span:        Span,
    /// Table schema inferred by the type-checker / resolver.
    /// `None` until semantic analysis has run.
    pub output_type: Option<Vec<(String, ColumnType)>>,
}

impl Step {
    pub fn new(kind: StepKind, span: Span) -> Self {
        Step { kind, span, output_type: None }
    }
}

/// One aggregation column produced by `Table.Group`.
#[derive(Debug, Clone)]
pub struct AggregateSpec {
    pub name:       String,
    pub expression: ExprNode,
    pub col_type:   ColumnType,
}

/// The kind of transformation a step performs.
///
/// # Generic FunctionCall design
///
/// All M function calls (Table.SelectRows, List.Transform, etc.) are
/// represented by the single `FunctionCall` variant.  The `name` field holds
/// the fully-qualified M function name and `args` holds the positionally-ordered
/// parsed arguments as `CallArg` values.
///
/// Only three variants remain as dedicated cases:
/// - `Source`        -- Excel.Workbook(...) initialises the source table
/// - `NavigateSheet` -- Source{[Item=...]}[Data] navigation expression
/// - `ValueBinding`  -- any non-function-call `let` binding
///
/// Adding a new M function requires:
///   1. An entry in pq_grammar::functions (arg hints + type signature)
///   2. One entry in the executor eval-registry
///   3. One entry in the SQL sql-registry
#[derive(Debug, Clone)]
pub enum StepKind {
    /// `Excel.Workbook(File.Contents("path"), optional useHeaders, optional delayTypes)`
    Source {
        path:        String,
        use_headers: Option<bool>,
        delay_types: Option<bool>,
    },

    /// All other M function calls -- the generic, data-driven case.
    ///
    /// `name` is the fully-qualified M function name (e.g. "Table.SelectRows").
    /// `args` holds the positionally-ordered parsed arguments as typed CallArg
    /// values that exactly match the ArgKind hints in the grammar registry.
    FunctionCall {
        name: String,
        args: Vec<CallArg>,
    },

    /// Workbook navigation: `Source{[Item="X", Kind="Sheet"]}[Data]`.
    NavigateSheet {
        input:      String,
        item:       String,
        sheet_kind: String,
        field:      String,
    },

    /// Direct value binding inside a `let` block -- scalar literals, lists,
    /// records, lambdas, arithmetic expressions.
    ValueBinding {
        expr: ExprNode,
    },
}

/// Sort direction.
#[derive(Debug, Clone, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl std::fmt::Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SortOrder::Ascending  => write!(f, "Order.Ascending"),
            SortOrder::Descending => write!(f, "Order.Descending"),
        }
    }
}

/// Join kind for `Table.Join`, `Table.NestedJoin`, etc.
#[derive(Debug, Clone, PartialEq)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
    LeftAnti,
    RightAnti,
}

impl std::fmt::Display for JoinKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            JoinKind::Inner      => write!(f, "JoinKind.Inner"),
            JoinKind::Left       => write!(f, "JoinKind.Left"),
            JoinKind::Right      => write!(f, "JoinKind.Right"),
            JoinKind::Full       => write!(f, "JoinKind.Full"),
            JoinKind::LeftAnti   => write!(f, "JoinKind.LeftAnti"),
            JoinKind::RightAnti  => write!(f, "JoinKind.RightAnti"),
        }
    }
}

/// Helper: extract the primary input step name from a FunctionCall arg list.
/// Conventionally the first arg is always a StepRef (the input table).
pub fn step_input(kind: &StepKind) -> &str {
    match kind {
        StepKind::Source { .. }   => "",
        StepKind::NavigateSheet { input, .. } => input.as_str(),
        StepKind::ValueBinding { .. } => "",
        StepKind::FunctionCall { args, .. } => {
            args.first()
                .and_then(|a| a.as_step_ref())
                .unwrap_or("")
        }
    }
}
