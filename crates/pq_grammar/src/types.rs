use std::collections::HashMap;
use std::fmt;

// ── Param – typed parameter with optional flag ────────────────────────────

/// A single parameter in a function signature.
///
/// Both required and optional arguments are represented here, so the type
/// system can express the full M parameter model (e.g.
/// `Table.AddIndexColumn(table, col, optional start, optional step)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// The semantic type of this parameter.
    pub ty:       Type,
    /// When `true` this parameter may be omitted at the call site.
    pub optional: bool,
}

impl Param {
    pub fn required(ty: Type) -> Self { Param { ty, optional: false } }
    pub fn opt(ty: Type)      -> Self { Param { ty, optional: true  } }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.optional {
            write!(f, "{}?", self.ty)
        } else {
            write!(f, "{}", self.ty)
        }
    }
}

// ── Core type algebra ─────────────────────────────────────────────────────

/// A type in the M / Power Query type system.
///
/// Used exclusively for semantic type-checking and generic instantiation;
/// the parser still uses the flat [`crate::functions::ArgKind`] layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Numeric scalar (Int64 or Number.Type)
    Number,
    /// Text / String
    Text,
    /// Logical (Boolean)
    Boolean,
    /// A full table value
    Table,
    /// A record (row) value
    Record,
    /// A homogeneous list of some inner type, e.g. `List<Text>`
    List(Box<Type>),
    /// `nullable X` – the value may be `X` or `null` (M's nullable type)
    Nullable(Box<Type>),
    /// Top type / wildcard – accepts or produces any value
    Any,
    /// A type variable for generic functions, e.g. `TypeVar("T")`
    TypeVar(String),
    /// A first-class function value, e.g. `(Record) → Boolean`
    Function(Box<FunctionType>),
}

// ── FunctionType ──────────────────────────────────────────────────────────

/// The signature of a callable: parameters (with optionality) + return type.
///
/// Used both for function-definition overloads and as the inner type of
/// `Type::Function` (in which case all params are required).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub params:      Vec<Param>,
    pub return_type: Box<Type>,
}

impl FunctionType {
    pub fn new(params: Vec<Param>, return_type: Type) -> Self {
        FunctionType {
            params,
            return_type: Box::new(return_type),
        }
    }

    /// Number of parameters that **must** be supplied at the call site.
    pub fn required_arity(&self) -> usize {
        self.params.iter().filter(|p| !p.optional).count()
    }

    /// Maximum total parameters (required + optional).
    pub fn max_arity(&self) -> usize {
        self.params.len()
    }

    /// Returns `true` when `n` supplied arguments is a valid arity for this
    /// signature (i.e. `required_arity ≤ n ≤ max_arity`).
    pub fn arity_matches(&self, n: usize) -> bool {
        n >= self.required_arity() && n <= self.max_arity()
    }
}

// ── Type methods ──────────────────────────────────────────────────────────

impl Type {
    /// Returns `true` if this is an unbound type variable.
    pub fn is_var(&self) -> bool {
        matches!(self, Type::TypeVar(_))
    }

    /// Collect all distinct free type-variable names reachable from this type.
    pub fn free_vars(&self) -> Vec<String> {
        let mut out = vec![];
        self.collect_vars(&mut out);
        out
    }

    fn collect_vars(&self, out: &mut Vec<String>) {
        match self {
            Type::TypeVar(v) => {
                if !out.contains(v) { out.push(v.clone()); }
            }
            Type::List(inner) | Type::Nullable(inner) => inner.collect_vars(out),
            Type::Function(ft) => {
                for p in &ft.params { p.ty.collect_vars(out); }
                ft.return_type.collect_vars(out);
            }
            _ => {}
        }
    }

    /// Apply a substitution map `{ TypeVar name → concrete Type }` recursively.
    pub fn substitute(&self, subst: &HashMap<String, Type>) -> Type {
        match self {
            Type::TypeVar(v) => subst.get(v).cloned().unwrap_or_else(|| self.clone()),
            Type::List(inner)     => Type::List(Box::new(inner.substitute(subst))),
            Type::Nullable(inner) => Type::Nullable(Box::new(inner.substitute(subst))),
            Type::Function(ft)    => Type::Function(Box::new(FunctionType {
                params:      ft.params.iter().map(|p| Param {
                    ty:       p.ty.substitute(subst),
                    optional: p.optional,
                }).collect(),
                return_type: Box::new(ft.return_type.substitute(subst)),
            })),
            other => other.clone(),
        }
    }
}

// ── Generic unification ───────────────────────────────────────────────────

/// Apply already-known substitutions to `ty` (single pass).
fn apply_subst(ty: &Type, subst: &HashMap<String, Type>) -> Type {
    match ty {
        Type::TypeVar(v) => subst.get(v).cloned().unwrap_or_else(|| ty.clone()),
        Type::List(inner)     => Type::List(Box::new(apply_subst(inner, subst))),
        Type::Nullable(inner) => Type::Nullable(Box::new(apply_subst(inner, subst))),
        Type::Function(ft)    => Type::Function(Box::new(FunctionType {
            params:      ft.params.iter().map(|p| Param {
                ty:       apply_subst(&p.ty, subst),
                optional: p.optional,
            }).collect(),
            return_type: Box::new(apply_subst(&ft.return_type, subst)),
        })),
        other => other.clone(),
    }
}

/// Unify two types, extending `subst` with any new variable bindings.
///
/// Returns `true` on success, `false` when the two types are structurally
/// incompatible.  `TypeVar` bindings are recorded in `subst` so that later
/// calls can resolve them.
///
/// `Nullable(T)` is compatible with `T` and vice-versa (M's permissive null
/// model), so we strip the nullable wrapper before comparing inner types.
pub fn unify(a: &Type, b: &Type, subst: &mut HashMap<String, Type>) -> bool {
    let a = apply_subst(a, subst);
    let b = apply_subst(b, subst);

    match (&a, &b) {
        // Top type: compatible with everything
        (Type::Any, _) | (_, Type::Any) => true,

        // Nullable: strip the wrapper and compare inner types so that
        //   `nullable T`  unifies with  `T`  and vice-versa.
        (Type::Nullable(ai), Type::Nullable(bi)) => unify(ai, bi, subst),
        (Type::Nullable(ai), _)                  => unify(ai, &b, subst),
        (_, Type::Nullable(bi))                  => unify(&a, bi, subst),

        // TypeVar on the left → bind
        (Type::TypeVar(v), rhs) => {
            if let Type::TypeVar(v2) = rhs { if v == v2 { return true; } }
            subst.insert(v.clone(), rhs.clone());
            true
        }
        // TypeVar on the right → bind
        (lhs, Type::TypeVar(v)) => {
            subst.insert(v.clone(), lhs.clone());
            true
        }

        // Ground-type structural matches
        (Type::Number,  Type::Number)  => true,
        (Type::Text,    Type::Text)    => true,
        (Type::Boolean, Type::Boolean) => true,
        (Type::Table,   Type::Table)   => true,
        (Type::Record,  Type::Record)  => true,

        (Type::List(ai), Type::List(bi)) => unify(ai, bi, subst),

        (Type::Function(af), Type::Function(bf)) => {
            if af.params.len() != bf.params.len() { return false; }
            for (ap, bp) in af.params.iter().zip(bf.params.iter()) {
                if !unify(&ap.ty, &bp.ty, subst) { return false; }
            }
            unify(&af.return_type, &bf.return_type, subst)
        }

        _ => false,
    }
}

// ── Builder helpers ───────────────────────────────────────────────────────

/// Required parameter (the primary building block for `sig`).
///
/// ```rust
/// use pq_grammar::types::{p, Type};
/// let param = p(Type::Table); // required Table param
/// ```
pub fn p(ty: Type) -> Param {
    Param::required(ty)
}

/// Optional parameter.
///
/// ```rust
/// use pq_grammar::types::{opt, Type};
/// let param = opt(Type::Number); // optional Number param
/// ```
pub fn opt(ty: Type) -> Param {
    Param::opt(ty)
}

/// Wrap a type as `Type::Nullable`.
///
/// ```rust
/// use pq_grammar::types::{nullable, Type};
/// let t = nullable(Type::Text); // nullable Text
/// ```
pub fn nullable(ty: Type) -> Type {
    Type::Nullable(Box::new(ty))
}

/// Build a `FunctionType` from plain types (all required).
///
/// This is the primary shorthand for the common case; every `Type` is wrapped
/// in a required [`Param`] automatically.
///
/// ```rust
/// use pq_grammar::types::{sig, Type};
/// let s = sig(vec![Type::Table, Type::Text], Type::Table);
/// ```
pub fn sig(param_types: Vec<Type>, ret: Type) -> FunctionType {
    FunctionType {
        params:      param_types.into_iter().map(Param::required).collect(),
        return_type: Box::new(ret),
    }
}

/// Build a `FunctionType` from explicit [`Param`] objects.
///
/// Use this variant when you need optional parameters or want to explicitly
/// control optionality.
///
/// ```rust
/// use pq_grammar::types::{sigp, p, opt, Type};
/// let s = sigp(vec![p(Type::Number), opt(Type::Number), opt(Type::Number)], Type::Number);
/// ```
pub fn sigp(params: Vec<Param>, ret: Type) -> FunctionType {
    FunctionType::new(params, ret)
}

/// Build a `Type::Function` from plain types (all required, anonymous params).
///
/// Convenient for writing inner function-type expressions such as
/// `fun(vec![Type::Record], Type::Boolean)` → `(Record) → Boolean`.
///
/// ```rust
/// use pq_grammar::types::{fun, Type};
/// let f = fun(vec![Type::Record], Type::Boolean);
/// ```
pub fn fun(param_types: Vec<Type>, ret: Type) -> Type {
    Type::Function(Box::new(FunctionType {
        params:      param_types.into_iter().map(Param::required).collect(),
        return_type: Box::new(ret),
    }))
}

/// Wrap an inner type as `Type::List`.
///
/// ```rust
/// use pq_grammar::types::{list, Type};
/// let t = list(Type::Text);
/// ```
pub fn list(inner: Type) -> Type {
    Type::List(Box::new(inner))
}

/// Create a named type variable.
///
/// ```rust
/// use pq_grammar::types::tvar;
/// let t = tvar("T");
/// ```
pub fn tvar(name: &str) -> Type {
    Type::TypeVar(name.to_string())
}

// ── Schema types ──────────────────────────────────────────────────────────

/// One column in a table's schema.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSchema {
    pub name: String,
    pub ty:   Type,
}

impl ColumnSchema {
    pub fn new(name: impl Into<String>, ty: Type) -> Self {
        ColumnSchema { name: name.into(), ty }
    }
}

/// The ordered schema of a table (list of column definitions).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableSchema {
    pub columns: Vec<ColumnSchema>,
}

impl TableSchema {
    pub fn new(columns: Vec<ColumnSchema>) -> Self { TableSchema { columns } }

    /// Look up a column by name.
    pub fn get_column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Builder: append a column.
    pub fn with_column(mut self, name: impl Into<String>, ty: Type) -> Self {
        self.columns.push(ColumnSchema::new(name, ty));
        self
    }

    /// Builder: keep only the listed column names (in given order).
    pub fn select_columns(self, names: &[&str]) -> Self {
        TableSchema {
            columns: names.iter()
                .filter_map(|n| self.get_column(n).cloned())
                .collect(),
        }
    }

    /// Builder: remove the listed column names.
    pub fn remove_columns(mut self, names: &[String]) -> Self {
        self.columns.retain(|c| !names.contains(&c.name));
        self
    }

    /// Builder: rename a column.
    pub fn rename_column(mut self, old: &str, new: &str) -> Self {
        for c in &mut self.columns {
            if c.name == old { c.name = new.to_string(); }
        }
        self
    }

    /// Builder: change the type of a column.
    pub fn retype_column(mut self, name: &str, ty: Type) -> Self {
        for c in &mut self.columns {
            if c.name == name { c.ty = ty.clone(); }
        }
        self
    }
}

// ── Schema-transform hook ─────────────────────────────────────────────────

/// Structural call-site arguments passed to a schema-transform function.
///
/// The checker fills this in from the parsed `StepKind` so that schema-aware
/// functions can compute their output column set statically.
#[derive(Debug, Clone, Default)]
pub struct SchemaTransformArgs {
    /// Known schema of the primary table input, if determinable at compile time.
    pub input_schema: Option<TableSchema>,
    /// String-valued arguments extracted from the call (e.g. new column names).
    pub str_args:     Vec<String>,
    /// Column rename pairs `(old_name, new_name)`.
    pub rename_pairs: Vec<(String, String)>,
    /// Column type-annotation pairs `(col_name, type)`.
    pub type_pairs:   Vec<(String, Type)>,
    /// Inferred return type of any function-valued argument (e.g. `each` expr).
    pub fn_return_type: Option<Type>,
}

/// A schema-transform callback used by schema-aware functions.
///
/// Given the [`SchemaTransformArgs`] assembled at the call site, the function
/// returns the output [`TableSchema`], or `None` if it cannot be determined
/// statically (e.g. `Table.PromoteHeaders`).
///
/// Uses a plain function pointer so that [`crate::functions::FunctionDef`]
/// remains `Clone + Copy`-friendly.
pub type SchemaTransformFn = fn(&SchemaTransformArgs) -> Option<TableSchema>;

// ── Display ───────────────────────────────────────────────────────────────

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Number          => write!(f, "Number"),
            Type::Text            => write!(f, "Text"),
            Type::Boolean         => write!(f, "Boolean"),
            Type::Table           => write!(f, "Table"),
            Type::Record          => write!(f, "Record"),
            Type::List(inner)     => write!(f, "List<{}>", inner),
            Type::Nullable(inner) => write!(f, "nullable {}", inner),
            Type::Any             => write!(f, "Any"),
            Type::TypeVar(v)      => write!(f, "{}", v),
            Type::Function(ft)    => write!(f, "{}", ft),
        }
    }
}

impl fmt::Display for FunctionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, p) in self.params.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{}", p)?;
        }
        write!(f, ") → {}", self.return_type)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── display ───────────────────────────────────────────────────────────

    #[test]
    fn test_display_simple() {
        assert_eq!(Type::Number.to_string(),  "Number");
        assert_eq!(Type::Text.to_string(),    "Text");
        assert_eq!(Type::Boolean.to_string(), "Boolean");
        assert_eq!(Type::Table.to_string(),   "Table");
        assert_eq!(Type::Record.to_string(),  "Record");
        assert_eq!(Type::Any.to_string(),     "Any");
    }

    #[test]
    fn test_display_list() {
        assert_eq!(list(Type::Text).to_string(),           "List<Text>");
        assert_eq!(list(list(Type::Number)).to_string(),   "List<List<Number>>");
    }

    #[test]
    fn test_display_nullable() {
        assert_eq!(nullable(Type::Text).to_string(),   "nullable Text");
        assert_eq!(nullable(Type::Number).to_string(), "nullable Number");
    }

    #[test]
    fn test_display_typevar() {
        assert_eq!(tvar("T").to_string(), "T");
    }

    #[test]
    fn test_display_function() {
        let f = fun(vec![Type::Record], Type::Boolean);
        assert_eq!(f.to_string(), "(Record) → Boolean");
    }

    #[test]
    fn test_display_function_with_optional_param() {
        let ft = sigp(vec![p(Type::Number), opt(Type::Number)], Type::Number);
        assert_eq!(ft.to_string(), "(Number, Number?) → Number");
    }

    #[test]
    fn test_display_generic_function() {
        let f = fun(
            vec![list(tvar("T")), fun(vec![tvar("T")], tvar("U"))],
            list(tvar("U")),
        );
        assert_eq!(f.to_string(), "(List<T>, (T) → U) → List<U>");
    }

    // ── Param ─────────────────────────────────────────────────────────────

    #[test]
    fn test_param_required_vs_optional() {
        let r = p(Type::Table);
        assert!(!r.optional);
        assert_eq!(r.ty, Type::Table);

        let o = opt(Type::Number);
        assert!(o.optional);
        assert_eq!(o.ty, Type::Number);
    }

    // ── arity helpers ─────────────────────────────────────────────────────

    #[test]
    fn test_arity_matches() {
        // Round: required=1, optional=2 → accepts 1, 2, 3 args
        let ft = sigp(vec![p(Type::Number), opt(Type::Number), opt(Type::Number)], Type::Number);
        assert_eq!(ft.required_arity(), 1);
        assert_eq!(ft.max_arity(),      3);
        assert!( ft.arity_matches(1));
        assert!( ft.arity_matches(2));
        assert!( ft.arity_matches(3));
        assert!(!ft.arity_matches(0));
        assert!(!ft.arity_matches(4));
    }

    // ── free_vars / substitute ────────────────────────────────────────────

    #[test]
    fn test_free_vars() {
        let t = fun(vec![list(tvar("T")), fun(vec![tvar("T")], tvar("U"))], list(tvar("U")));
        let mut vars = t.free_vars();
        vars.sort();
        assert_eq!(vars, vec!["T", "U"]);
    }

    #[test]
    fn test_substitute() {
        let mut subst = HashMap::new();
        subst.insert("T".to_string(), Type::Number);
        subst.insert("U".to_string(), Type::Text);

        let t = fun(vec![list(tvar("T")), fun(vec![tvar("T")], tvar("U"))], list(tvar("U")));
        let concrete = t.substitute(&subst);

        let expected = fun(
            vec![list(Type::Number), fun(vec![Type::Number], Type::Text)],
            list(Type::Text),
        );
        assert_eq!(concrete, expected);
    }

    #[test]
    fn test_substitute_nullable() {
        let mut subst = HashMap::new();
        subst.insert("T".to_string(), Type::Text);
        let t = nullable(tvar("T"));
        assert_eq!(t.substitute(&subst), nullable(Type::Text));
    }

    // ── unify: ground types ───────────────────────────────────────────────

    #[test]
    fn test_unify_ground_types() {
        let mut s = HashMap::new();
        assert!( unify(&Type::Number,  &Type::Number,  &mut s));
        assert!( unify(&Type::Text,    &Type::Text,    &mut s));
        assert!( unify(&Type::Boolean, &Type::Boolean, &mut s));
        assert!(!unify(&Type::Number,  &Type::Text,    &mut s));
    }

    #[test]
    fn test_unify_any() {
        let mut s = HashMap::new();
        assert!(unify(&Type::Any, &Type::Number, &mut s));
        assert!(unify(&Type::Text, &Type::Any,  &mut s));
    }

    // ── unify: Nullable ───────────────────────────────────────────────────

    #[test]
    fn test_unify_nullable_nullable() {
        let mut s = HashMap::new();
        assert!(unify(&nullable(Type::Text), &nullable(Type::Text), &mut s));
        assert!(!unify(&nullable(Type::Text), &nullable(Type::Number), &mut s));
    }

    #[test]
    fn test_unify_nullable_strips_wrapper() {
        // nullable Text  unifies with  Text  (and vice-versa)
        let mut s = HashMap::new();
        assert!(unify(&nullable(Type::Text), &Type::Text, &mut s));

        let mut s2 = HashMap::new();
        assert!(unify(&Type::Text, &nullable(Type::Text), &mut s2));
    }

    #[test]
    fn test_unify_nullable_with_typevar() {
        let mut s = HashMap::new();
        // nullable T  unifies with  Number  → binds T = Number
        assert!(unify(&nullable(tvar("T")), &Type::Number, &mut s));
        assert_eq!(s.get("T"), Some(&Type::Number));
    }

    // ── unify: TypeVar ────────────────────────────────────────────────────

    #[test]
    fn test_unify_typevar_binds() {
        let mut s = HashMap::new();
        assert!(unify(&tvar("T"), &Type::Number, &mut s));
        assert_eq!(s.get("T"), Some(&Type::Number));
    }

    #[test]
    fn test_unify_typevar_right() {
        let mut s = HashMap::new();
        assert!(unify(&Type::Text, &tvar("T"), &mut s));
        assert_eq!(s.get("T"), Some(&Type::Text));
    }

    #[test]
    fn test_unify_list() {
        let mut s = HashMap::new();
        assert!(unify(&list(tvar("T")), &list(Type::Number), &mut s));
        assert_eq!(s.get("T"), Some(&Type::Number));
    }

    #[test]
    fn test_unify_function() {
        let mut s = HashMap::new();
        let lhs = fun(vec![tvar("T")], tvar("U"));
        let rhs = fun(vec![Type::Record], Type::Boolean);
        assert!(unify(&lhs, &rhs, &mut s));
        assert_eq!(s.get("T"), Some(&Type::Record));
        assert_eq!(s.get("U"), Some(&Type::Boolean));
    }

    // ── semantic examples ─────────────────────────────────────────────────

    #[test]
    fn test_select_rows_signature() {
        // (Table, (Record) → Boolean) → Table
        let mut s = HashMap::new();
        let sig_ty = fun(
            vec![Type::Table, fun(vec![Type::Record], Type::Boolean)],
            Type::Table,
        );
        let call_arg2 = fun(vec![Type::Record], Type::Boolean);
        if let Type::Function(ft) = &sig_ty {
            assert!(unify(&ft.params[0].ty, &Type::Table, &mut s));
            assert!(unify(&ft.params[1].ty, &call_arg2,   &mut s));
            assert_eq!(*ft.return_type, Type::Table);
        }
    }

    #[test]
    fn test_list_transform_generic() {
        // (List<T>, (T) → U) → List<U>
        let s = sigp(
            vec![p(list(tvar("T"))), p(fun(vec![tvar("T")], tvar("U")))],
            list(tvar("U")),
        );
        let mut subst = HashMap::new();
        unify(&s.params[0].ty, &list(Type::Number), &mut subst);
        unify(&s.params[1].ty, &fun(vec![Type::Number], Type::Text), &mut subst);
        let ret = s.return_type.substitute(&subst);
        assert_eq!(ret, list(Type::Text));
    }

    // ── TableSchema ───────────────────────────────────────────────────────

    #[test]
    fn test_table_schema_builders() {
        let schema = TableSchema::default()
            .with_column("Name",   Type::Text)
            .with_column("Age",    Type::Number)
            .with_column("Active", Type::Boolean);

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.get_column("Age").unwrap().ty, Type::Number);

        let removed = schema.clone().remove_columns(&["Age".to_string()]);
        assert!(removed.get_column("Age").is_none());
        assert_eq!(removed.columns.len(), 2);

        let renamed = schema.clone().rename_column("Name", "FullName");
        assert!(renamed.get_column("Name").is_none());
        assert!(renamed.get_column("FullName").is_some());

        let retyped = schema.clone().retype_column("Age", nullable(Type::Number));
        assert_eq!(
            retyped.get_column("Age").unwrap().ty,
            nullable(Type::Number)
        );
    }

    #[test]
    fn test_schema_select_columns() {
        let schema = TableSchema::default()
            .with_column("A", Type::Text)
            .with_column("B", Type::Number)
            .with_column("C", Type::Boolean);
        let sel = schema.select_columns(&["C", "A"]);
        assert_eq!(sel.columns.len(), 2);
        assert_eq!(sel.columns[0].name, "C");
        assert_eq!(sel.columns[1].name, "A");
    }
}
