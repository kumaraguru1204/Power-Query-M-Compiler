use pq_diagnostics::Span;

/// A single entry in the scope.
/// Tracks a step name and where it was defined.
#[derive(Debug, Clone)]
pub struct ScopeEntry {
    /// the step name: "Source", "FilteredRows" etc.
    pub name: String,

    /// where this step was defined in the source
    pub defined_at: Span,
}

/// The scope tracks which step names are visible
/// at any point during resolution.
///
/// Steps are added one by one as we walk through
/// the program in source order.
/// A step can only reference names that were added
/// before it — this enforces sequential ordering.
#[derive(Debug, Default)]
pub struct Scope {
    entries: Vec<ScopeEntry>,
}

impl Scope {
    pub fn new() -> Self {
        Scope { entries: vec![] }
    }

    /// Add a new step name to the scope.
    pub fn define(&mut self, name: String, defined_at: Span) {
        self.entries.push(ScopeEntry { name, defined_at });
    }

    /// Check if a name exists in the scope.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }

    /// Find an entry by name.
    pub fn get(&self, name: &str) -> Option<&ScopeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// All names currently in scope, in definition order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// Find the closest name to a given string.
    /// Used for "did you mean?" suggestions.
    pub fn closest_match(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .filter_map(|e| {
                let dist = edit_distance(&e.name, name);
                if dist <= 3 { Some((dist, e.name.as_str())) } else { None }
            })
            .min_by_key(|(dist, _)| *dist)
            .map(|(_, name)| name)
    }
}

/// Simple edit distance for "did you mean?" suggestions.
/// Uses dynamic programming (Wagner-Fischer algorithm).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }

    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }

    dp[m][n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_define_and_contains() {
        let mut scope = Scope::new();
        scope.define("Source".into(), pq_diagnostics::Span::dummy());
        assert!(scope.contains("Source"));
        assert!(!scope.contains("Missing"));
    }

    #[test]
    fn test_closest_match() {
        let mut scope = Scope::new();
        scope.define("Source".into(),          pq_diagnostics::Span::dummy());
        scope.define("PromotedHeaders".into(), pq_diagnostics::Span::dummy());
        scope.define("ChangedTypes".into(),    pq_diagnostics::Span::dummy());

        // "Sourec" is close to "Source"
        assert_eq!(scope.closest_match("Sourec"), Some("Source"));
    }

    #[test]
    fn test_edit_distance() {
        assert_eq!(edit_distance("Source", "Source"), 0);
        assert_eq!(edit_distance("Source", "Sourec"), 2);
        assert_eq!(edit_distance("",       "abc"),    3);
    }
}