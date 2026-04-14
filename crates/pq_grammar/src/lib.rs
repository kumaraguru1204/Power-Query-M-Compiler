pub mod keywords;
pub mod operators;
pub mod functions;
pub mod types;

pub use functions::{
    NamespaceDef,
    FunctionDef,
    ArgKind,
    registry,
    lookup_namespace,
    lookup_function,
    lookup_qualified,
    functions_in_namespace,
    all_qualified_names,
};

pub use types::{
    // Core type algebra
    Type,
    FunctionType,
    Param,
    // Unification
    unify,
    // Builder helpers
    sig,
    sigp,
    fun,
    list,
    tvar,
    p,
    opt,
    nullable,
    // Schema types
    ColumnSchema,
    TableSchema,
    SchemaTransformArgs,
    SchemaTransformFn,
};
