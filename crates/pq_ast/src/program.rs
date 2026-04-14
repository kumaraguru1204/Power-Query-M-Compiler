use pq_diagnostics::Span;
use crate::Step;

/// A named step binding.
/// In M:  StepName = Table.SelectRows(...)
#[derive(Debug, Clone)]
pub struct StepBinding {
    /// the name given to this step: "FilteredRows"
    pub name: String,

    /// where the name appears in the source
    pub name_span: Span,

    /// the step itself
    pub step: Step,
}

impl StepBinding {
    pub fn new(name: String, name_span: Span, step: Step) -> Self {
        StepBinding { name, name_span, step }
    }
}

/// A complete M-like program.
///
/// Grammar:
///   program   = "let" step_list "in" identifier
///   step_list = binding { "," binding }
///   binding   = identifier "=" call_expr
#[derive(Debug)]
pub struct Program {
    /// all named step bindings in source order
    pub steps: Vec<StepBinding>,

    /// the name of the final step to return
    pub output: String,

    /// where the output name appears in the source
    pub output_span: Span,
}

impl Program {
    /// Find a step binding by name.
    pub fn get_step(&self, name: &str) -> Option<&StepBinding> {
        self.steps.iter().find(|s| s.name == name)
    }

    /// All step names in order.
    pub fn step_names(&self) -> Vec<&str> {
        self.steps.iter().map(|s| s.name.as_str()).collect()
    }

    /// Does a step with this name exist?
    pub fn has_step(&self, name: &str) -> bool {
        self.steps.iter().any(|s| s.name == name)
    }
}