//! Dumps the function catalogue as a TSV table:
//!   namespace<TAB>name<TAB>min_arity<TAB>max_arity<TAB>arg_hints<TAB>schema_hook<TAB>signature<TAB>doc
//!
//! Run with: `cargo run -p pq_grammar --example dump_catalogue`
//!
//! Used to regenerate `specs/cross_cutting/16_function_catalogue.md`.
use pq_grammar::{registry, ArgKind};

fn arg_hint_str(k: &ArgKind) -> &'static str {
    match k {
        ArgKind::StringLit => "str",
        ArgKind::StepRef => "step",
        ArgKind::EachExpr => "each",
        ArgKind::TypeList => "typelist",
        ArgKind::ColumnList => "cols",
        ArgKind::RenameList => "rename",
        ArgKind::SortList => "sort",
        ArgKind::Integer => "int",
        ArgKind::Value => "val",
        ArgKind::RecordLit => "rec",
        ArgKind::RecordList => "reclist",
        ArgKind::AggregateList => "agg",
        ArgKind::JoinKind => "join",
        ArgKind::TransformList => "xform",
        ArgKind::StepRefList => "steplist",
        ArgKind::OptInteger => "int?",
        ArgKind::OptValue => "val?",
        ArgKind::OptRecordLit => "rec?",
        ArgKind::OptJoinKind => "join?",
        ArgKind::StepRefOrValue => "step|val",
        ArgKind::FileContentsArg => "filepath",
        ArgKind::OptNullableBool => "nbool?",
        ArgKind::ColumnListOrString => "cols|str",
        ArgKind::OptMissingField => "missing?",
        ArgKind::BareTypeList => "btypelist",
        ArgKind::OptCultureOrRecord => "culture|rec?",
    }
}

fn main() {
    println!("namespace\tname\tmin_arity\tmax_arity\tn_overloads\targ_hints\tschema_hook\tprimary_signature\tdoc");
    for ns in registry() {
        for f in &ns.functions {
            let hints = f.arg_hints.iter().map(arg_hint_str).collect::<Vec<_>>().join(",");
            let prim = f.primary_sig();
            let min_ar = prim.required_arity();
            let max_ar = prim.max_arity();
            let hook = if f.schema_transform.is_some() { "yes" } else { "" };
            let sig_ascii = format!("{}", prim).replace('\u{2192}', "->");
            let doc = f.doc.replace('\t', " ").replace('\n', " ")
                .replace('\u{2192}', "->")
                .replace('\u{2014}', "-")
                .replace('\u{2013}', "-");
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                ns.name, f.name, min_ar, max_ar, f.signatures.len(), hints, hook, sig_ascii, doc
            );
        }
    }
}
