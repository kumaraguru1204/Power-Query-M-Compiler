use pq_diagnostics::Span;
use pq_types::ColumnType;
use crate::ExprNode;

/// A single named transformation step.
/// Maps directly to one M function call.
///
/// Every step carries a Span pointing to where
/// it appears in the source formula.
#[derive(Debug, Clone)]
pub struct Step {
    pub kind: StepKind,
    pub span: Span,
    /// Table schema inferred by the type-checker / resolver.
    /// `None` until semantic analysis has run.
    /// Each element is `(column_name, column_type)`.
    pub output_type: Option<Vec<(String, ColumnType)>>,
}

impl Step {
    pub fn new(kind: StepKind, span: Span) -> Self {
        Step { kind, span, output_type: None }
    }
}

/// One aggregation column produced by `Table.Group`.
///
/// Corresponds to one `{"name", each expr, Type}` triple in the M formula.
/// The `expression` field is a `Lambda { param: "_", body }` node.
#[derive(Debug, Clone)]
pub struct AggregateSpec {
    /// Output column name.
    pub name:       String,
    /// The aggregation expression — an `Each(inner)` node.
    pub expression: ExprNode,
    /// Declared output type.
    pub col_type:   ColumnType,
}

/// The kind of transformation a step performs.
/// Each variant maps to one M function call.
#[derive(Debug, Clone)]
pub enum StepKind {
    /// `Excel.Workbook(File.Contents("path"), optional useHeaders, optional delayTypes)`
    /// Load a workbook from a file.
    ///
    /// `use_headers`: whether to promote the first row as headers (`null` = false).
    /// `delay_types`: whether to leave columns untyped (`null` = false).
    Source {
        path:        String,
        use_headers: Option<bool>,
        delay_types: Option<bool>,
    },

    /// `Table.PromoteHeaders(input)`
    /// Promote the first row to column headers.
    PromoteHeaders {
        input: String,
    },

    /// `Table.TransformColumnTypes(input, {{"Col", Type}, ...})`
    /// Change the type of one or more columns.
    ChangeTypes {
        input:   String,
        columns: Vec<(String, ColumnType)>,
    },

    /// `Table.SelectRows(input, each condition)`
    ///
    /// `condition` is a `Lambda { param: "_", body: bool_expr }` node.
    Filter {
        input:     String,
        condition: ExprNode,
    },

    /// `Table.AddColumn(input, "NewCol", each expression)`
    ///
    /// `expression` is a `Lambda { param: "_", body: expr }` node.
    AddColumn {
        input:      String,
        col_name:   String,
        expression: ExprNode,
    },

    /// `Table.RemoveColumns(input, {"Col1", "Col2"})`
    /// Remove one or more columns.
    RemoveColumns {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.RenameColumns(input, {{"OldName", "NewName"}})`
    /// Rename one or more columns.
    RenameColumns {
        input:   String,
        renames: Vec<(String, String)>,
    },

    /// `Table.Sort(input, {{"Col", Order.Ascending}})`
    /// Sort rows by one or more columns.
    Sort {
        input: String,
        by:    Vec<(String, SortOrder)>,
    },

    /// `Table.TransformColumns(input, {{"col", each expr}, ...})`\n    /// or `Table.TransformColumns(input, {{"col", each expr, type}, ...})`
    ///
    /// Each transform expression is a `Lambda { param: "_", body }` node.
    /// The optional type annotation specifies the target column type.
    TransformColumns {
        input:      String,
        transforms: Vec<(String, ExprNode, Option<ColumnType>)>,
    },

    /// `Table.Group(input, {"key_col"}, {{"name", each expr, Type}, ...})`
    Group {
        input:      String,
        by:         Vec<String>,
        aggregates: Vec<AggregateSpec>,
    },

    // ── Simple row operations ─────────────────────────────────────────

    /// `Table.FirstN(input, n)` — keep first n rows.
    FirstN {
        input: String,
        count: ExprNode,
    },

    /// `Table.LastN(input, n)` — keep last n rows.
    LastN {
        input: String,
        count: ExprNode,
    },

    /// `Table.Skip(input, n)` — skip first n rows.
    Skip {
        input: String,
        count: ExprNode,
    },

    /// `Table.Range(input, offset, count)` — rows from offset for count.
    Range {
        input:  String,
        offset: ExprNode,
        count:  ExprNode,
    },

    /// `Table.RemoveFirstN(input, n)` — remove first n rows.
    RemoveFirstN {
        input: String,
        count: ExprNode,
    },

    /// `Table.RemoveLastN(input, n)` — remove last n rows.
    RemoveLastN {
        input: String,
        count: ExprNode,
    },

    /// `Table.RemoveRows(input, offset, count)` — remove rows at offset.
    RemoveRows {
        input:  String,
        offset: ExprNode,
        count:  ExprNode,
    },

    /// `Table.ReverseRows(input)` — reverse row order.
    ReverseRows {
        input: String,
    },

    /// `Table.Distinct(input, {cols})` — deduplicate rows.
    Distinct {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.Repeat(input, n)` — repeat table n times.
    Repeat {
        input: String,
        count: ExprNode,
    },

    /// `Table.AlternateRows(input, offset, skip, take)` — odd/even row selection.
    AlternateRows {
        input:  String,
        offset: ExprNode,
        skip:   ExprNode,
        take:   ExprNode,
    },

    /// `Table.FindText(input, text)` — rows containing text.
    FindText {
        input: String,
        text:  String,
    },

    /// `Table.FillDown(input, {cols})` — fill null values downwards.
    FillDown {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.FillUp(input, {cols})` — fill null values upwards.
    FillUp {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.AddIndexColumn(input, "col", optional start, optional step)`
    AddIndexColumn {
        input:    String,
        col_name: String,
        start:    i64,
        step:     i64,
    },

    /// `Table.DuplicateColumn(input, col, newCol)`
    DuplicateColumn {
        input:    String,
        src_col:  String,
        new_col:  String,
    },

    /// `Table.Unpivot(input, {cols}, attrCol, valCol)`
    Unpivot {
        input:    String,
        columns:  Vec<String>,
        attr_col: String,
        val_col:  String,
    },

    /// `Table.UnpivotOtherColumns(input, {keepCols}, attrCol, valCol)`
    UnpivotOtherColumns {
        input:     String,
        keep_cols: Vec<String>,
        attr_col:  String,
        val_col:   String,
    },

    /// `Table.Transpose(input)`
    Transpose {
        input: String,
    },

    /// `Table.Combine({t1, t2, ...})` — concatenate tables.
    CombineTables {
        inputs: Vec<String>,
    },

    /// `Table.RemoveRowsWithErrors(input, {cols})`
    RemoveRowsWithErrors {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.SelectRowsWithErrors(input, {cols})`
    SelectRowsWithErrors {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.TransformRows(input, each expr)`
    TransformRows {
        input:     String,
        transform: ExprNode,
    },

    /// `Table.MatchesAllRows(input, each predicate)` → Boolean
    MatchesAllRows {
        input:     String,
        condition: ExprNode,
    },

    /// `Table.MatchesAnyRows(input, each predicate)` → Boolean
    MatchesAnyRows {
        input:     String,
        condition: ExprNode,
    },

    /// `Table.PrefixColumns(input, prefix)`
    PrefixColumns {
        input:  String,
        prefix: String,
    },

    /// `Table.DemoteHeaders(input)`
    DemoteHeaders {
        input: String,
    },

    // ── Column operations ─────────────────────────────────────────────

    /// `Table.SelectColumns(input, {col,...})` — keep only listed columns.
    SelectColumns {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.ReorderColumns(input, {col,...})` — reorder columns.
    ReorderColumns {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.TransformColumnNames(input, each expr)` — rename columns via function.
    TransformColumnNames {
        input:     String,
        transform: ExprNode,
    },

    /// `Table.CombineColumns(input, {cols}, each combiner, newCol)` — merge columns.
    CombineColumns {
        input:    String,
        columns:  Vec<String>,
        combiner: ExprNode,
        new_col:  String,
    },

    /// `Table.SplitColumn(input, col, each splitter)` — split a column.
    SplitColumn {
        input:    String,
        col_name: String,
        splitter: ExprNode,
    },

    /// `Table.ExpandTableColumn(input, col, {subCols})` — expand nested table column.
    ExpandTableColumn {
        input:   String,
        col_name: String,
        columns: Vec<String>,
    },

    /// `Table.ExpandRecordColumn(input, col, {fields})` — expand nested record column.
    ExpandRecordColumn {
        input:   String,
        col_name: String,
        fields:  Vec<String>,
    },

    /// `Table.Pivot(input, pivotCol, attrCol, valCol)` — pivot rows to columns.
    Pivot {
        input:     String,
        pivot_col: Vec<String>,
        attr_col:  String,
        val_col:   String,
    },

    // ── Information (scalar returns) ──────────────────────────────────

    /// `Table.RowCount(input)` → integer
    RowCount { input: String },

    /// `Table.ColumnCount(input)` → integer
    ColumnCount { input: String },

    /// `Table.ColumnNames(input)` → list of text
    TableColumnNames { input: String },

    /// `Table.IsEmpty(input)` → boolean
    TableIsEmpty { input: String },

    /// `Table.Schema(input)` → schema table
    TableSchema { input: String },

    // ── Membership (boolean returns) ─────────────────────────────────

    /// `Table.HasColumns(input, {col,...})` → boolean
    HasColumns {
        input:   String,
        columns: Vec<String>,
    },

    /// `Table.IsDistinct(input)` → boolean
    TableIsDistinct { input: String },

    // ── Joins ─────────────────────────────────────────────────────────

    /// `Table.Join(left, {leftKeys}, right, {rightKeys}, JoinKind.X)`
    Join {
        left:       String,
        left_keys:  Vec<String>,
        right:      String,
        right_keys: Vec<String>,
        join_kind:  JoinKind,
    },

    /// `Table.NestedJoin(left, {leftKeys}, right, {rightKeys}, newCol, JoinKind.X)`
    NestedJoin {
        left:       String,
        left_keys:  Vec<String>,
        right:      String,
        right_keys: Vec<String>,
        new_col:    String,
        join_kind:  JoinKind,
    },

    // ── Ordering ──────────────────────────────────────────────────────

    /// `Table.AddRankColumn(input, newCol, {{col, Order.X}})` — add rank column.
    AddRankColumn {
        input:    String,
        col_name: String,
        by:       Vec<(String, SortOrder)>,
    },

    /// `Table.Max(input, col)` → single row (record).
    TableMax {
        input:    String,
        col_name: String,
    },

    /// `Table.Min(input, col)` → single row (record).
    TableMin {
        input:    String,
        col_name: String,
    },

    /// `Table.MaxN(input, n, col)` → top-N rows.
    TableMaxN {
        input:    String,
        count:    ExprNode,
        col_name: String,
    },

    /// `Table.MinN(input, n, col)` → bottom-N rows.
    TableMinN {
        input:    String,
        count:    ExprNode,
        col_name: String,
    },

    // ── Value operations ──────────────────────────────────────────────

    /// `Table.ReplaceValue(input, old, new, each replacer)`
    ReplaceValue {
        input:     String,
        old_value: ExprNode,
        new_value: ExprNode,
        replacer:  ExprNode,
    },

    /// `Table.ReplaceErrorValues(input, {{col, replacement}, ...})`
    ReplaceErrorValues {
        input:        String,
        replacements: Vec<(String, ExprNode, Option<ColumnType>)>,
    },

    /// `Table.InsertRows(input, offset, {records})`
    InsertRows {
        input:  String,
        offset: i64,
    },

    /// `List.Generate(initial, condition, next, optional selector)`
    ///
    /// Generates a list by repeatedly applying `next` while `condition` holds,
    /// starting from the value produced by `initial`.
    ///
    /// * `initial`   — zero-arg lambda `() => seed` (or any expression)
    /// * `condition` — one-arg predicate `each _ < limit`
    /// * `next`      — one-arg step `each _ + 1`
    /// * `selector`  — optional one-arg projection `each _ * 2`
    ListGenerate {
        initial:   ExprNode,
        condition: ExprNode,
        next:      ExprNode,
        selector:  Option<ExprNode>,
    },

    /// `List.Transform(list_expr, each transform_fn)`
    ///
    /// Apply a per-element function to a list.  The result is a
    /// single-column table named `"Value"`.
    ///
    /// Unlike table operations, the first argument may be an inline list
    /// literal `{1, 2, 3}` **or** a step-reference identifier.
    ListTransform {
        /// The source list — an inline `{…}` literal, a step-reference
        /// `Identifier`, or any other expression that produces a list.
        list_expr: ExprNode,
        /// The per-element transformation lambda
        /// (`each` desugars to `Lambda { param: "_", body }`).
        transform: ExprNode,
    },

    /// Passthrough — input is forwarded unchanged.
    ///
    /// Used for functions that are registered in the grammar registry
    /// but do not yet have a full StepKind implementation.
    /// `func_name` carries the original qualified name (e.g. "Table.Buffer")
    /// so the formatter can reconstruct a syntactically valid M expression.
    Passthrough {
        /// Name of the input step this step reads from.
        input:     String,
        /// Original qualified function name, e.g. "Table.Buffer".
        func_name: String,
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
