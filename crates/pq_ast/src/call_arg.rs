use pq_types::ColumnType;
use crate::ExprNode;
use crate::step::{SortOrder, JoinKind, MissingFieldKind, AggregateSpec};

/// One parsed argument to a step-level M function call.
///
/// The `ArgKind` hints in `pq_grammar::FunctionDef` drive which variant is
/// produced for each positional argument. Downstream passes (executor, SQL
/// generator, formatter) extract the concrete data by calling the `as_*`
/// helper methods on the variant they expect.
///
/// Using a typed union here instead of a uniform `Vec<ExprNode>` means that
/// complex argument shapes like type-lists, sort-lists, and aggregate
/// specifications can be stored efficiently without re-parsing them.
#[derive(Debug, Clone)]
pub enum CallArg {
    // ── table / step references ───────────────────────────────────────────

    /// A bare identifier that names an upstream step, e.g. `Source`.
    StepRef(String),

    /// A brace-enclosed list of step-reference identifiers:
    /// `{Source, OtherTable}` — used by `Table.Combine`.
    StepRefList(Vec<String>),

    // ── general expressions ───────────────────────────────────────────────

    /// Any full expression: lambda (`each … / (x) => …`), inline list
    /// literal (`{1,2,3}`), nested function call, arithmetic, etc.
    Expr(ExprNode),

    // ── string arguments ──────────────────────────────────────────────────

    /// A string literal argument, e.g. a column name `"Age"` or a file path.
    Str(String),

    // ── column / rename / type metadata ──────────────────────────────────

    /// A brace-enclosed list of column-name strings:
    /// `{"Col1", "Col2"}` — used by many `Table.*` functions.
    ColList(Vec<String>),

    /// A brace-enclosed list of `{{"OldName", "NewName"}}` pairs:
    /// used by `Table.RenameColumns`.
    RenameList(Vec<(String, String)>),

    /// A brace-enclosed list of `{{"ColName", SomeType}}` pairs:
    /// used by `Table.TransformColumnTypes`.
    TypeList(Vec<(String, ColumnType)>),

    /// A bare type list `{type number, type text, …}` (no column names):
    /// used by `Table.ColumnsOfType`.
    BareTypeList(Vec<ColumnType>),

    // ── sort / join / aggregate metadata ─────────────────────────────────

    /// A sort-spec list `{{"Col", Order.Ascending}, …}`:
    /// used by `Table.Sort`, `Table.AddRankColumn`, etc.
    SortList(Vec<(String, SortOrder)>),

    /// A `JoinKind.Inner` / `JoinKind.Left` / … literal:
    /// used by `Table.Join`, `Table.NestedJoin`.
    JoinKindArg(JoinKind),

    /// A list of aggregate specs `{{"name", each expr, Type.X}, …}`:
    /// used by `Table.Group`.
    AggList(Vec<AggregateSpec>),

    /// A transform list `{{"col", each expr}, …}` or `{{"col", each expr, Type.X}, …}`:
    /// used by `Table.TransformColumns` and `Table.ReplaceErrorValues`.
    TransformList(Vec<(String, ExprNode, Option<ColumnType>)>),

    // ── scalar / optional arguments ───────────────────────────────────────

    /// A required integer literal argument.
    Int(i64),

    /// An optional integer (from `ArgKind::OptInteger`).
    /// `None` when the argument was absent.
    OptInt(Option<i64>),

    /// A nullable boolean (`null` | `true` | `false`):
    /// used by `Excel.Workbook`'s `useHeaders` / `delayTypes` params.
    NullableBool(Option<bool>),

    /// An optional `MissingField.Error / Ignore / UseNull` literal.
    OptMissingField(Option<MissingFieldKind>),

    /// The optional culture / options record for `Table.TransformColumnTypes`.
    /// Carries the culture string and any `MissingField` override.
    OptCulture(Option<String>, Option<MissingFieldKind>),
}

impl CallArg {
    // ── required accessors ────────────────────────────────────────────────

    /// Borrow the step-reference name, or panic/error if this variant isn't
    /// `StepRef`.  Used in eval fns where the first arg is always a table ref.
    pub fn as_step_ref(&self) -> Option<&str> {
        if let CallArg::StepRef(s) = self { Some(s.as_str()) } else { None }
    }

    pub fn as_step_ref_list(&self) -> Option<&[String]> {
        if let CallArg::StepRefList(v) = self { Some(v) } else { None }
    }

    pub fn as_expr(&self) -> Option<&ExprNode> {
        if let CallArg::Expr(e) = self { Some(e) } else { None }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let CallArg::Str(s) = self { Some(s.as_str()) } else { None }
    }

    pub fn as_col_list(&self) -> Option<&[String]> {
        if let CallArg::ColList(v) = self { Some(v) } else { None }
    }

    pub fn as_rename_list(&self) -> Option<&[(String, String)]> {
        if let CallArg::RenameList(v) = self { Some(v) } else { None }
    }

    pub fn as_type_list(&self) -> Option<&[(String, ColumnType)]> {
        if let CallArg::TypeList(v) = self { Some(v) } else { None }
    }

    pub fn as_bare_type_list(&self) -> Option<&[ColumnType]> {
        if let CallArg::BareTypeList(v) = self { Some(v) } else { None }
    }

    pub fn as_sort_list(&self) -> Option<&[(String, SortOrder)]> {
        if let CallArg::SortList(v) = self { Some(v) } else { None }
    }

    pub fn as_join_kind(&self) -> Option<&JoinKind> {
        if let CallArg::JoinKindArg(j) = self { Some(j) } else { None }
    }

    pub fn as_agg_list(&self) -> Option<&[AggregateSpec]> {
        if let CallArg::AggList(v) = self { Some(v) } else { None }
    }

    pub fn as_transform_list(&self) -> Option<&[(String, ExprNode, Option<ColumnType>)]> {
        if let CallArg::TransformList(v) = self { Some(v) } else { None }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            CallArg::Int(n)    => Some(*n),
            CallArg::OptInt(n) => *n,
            _ => None,
        }
    }

    pub fn as_nullable_bool(&self) -> Option<Option<bool>> {
        if let CallArg::NullableBool(b) = self { Some(*b) } else { None }
    }

    pub fn as_opt_missing_field(&self) -> Option<Option<&MissingFieldKind>> {
        if let CallArg::OptMissingField(m) = self { Some(m.as_ref()) } else { None }
    }

    pub fn as_opt_culture(&self) -> Option<(&Option<String>, &Option<MissingFieldKind>)> {
        if let CallArg::OptCulture(s, m) = self { Some((s, m)) } else { None }
    }

    // ── convenience: unwrap with fallback ─────────────────────────────────

    /// Return the step-ref name or empty string (for functions where the
    /// input may have been a nested call that wasn't fully resolved).
    pub fn step_ref_or_empty(&self) -> &str {
        self.as_step_ref().unwrap_or("")
    }
}
