use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A typed value produced at runtime when evaluating an expression.
/// All table values start as raw strings; the executor coerces them
/// to the right variant based on the column's inferred ColumnType.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Null,
    /// Heterogeneous list value, e.g. `{1, 2, 3}` or `{[Name="A"], [Name="B"]}`.
    List(Vec<Value>),
    /// Record value, e.g. `[Name = "Alice", Age = 30]`. Field order is
    /// preserved by `BTreeMap` for deterministic iteration.
    Record(BTreeMap<String, Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a),   Value::Int(b))   => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            // cross-type numeric equality
            (Value::Int(a),   Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b))   => *a == (*b as f64),
            (Value::Bool(a),  Value::Bool(b))  => a == b,
            (Value::Text(a),  Value::Text(b))  => a == b,
            (Value::Null,     Value::Null)      => true,
            (Value::List(a),  Value::List(b))  => a == b,
            (Value::Record(a),Value::Record(b)) => a == b,
            _                                   => false,
        }
    }
}

impl Value {
    /// Convert back to the raw string stored in the Table column.
    pub fn to_raw_string(&self) -> String {
        match self {
            Value::Int(n)    => n.to_string(),
            Value::Float(n)  => {
                // always keep decimal point so "5000.0" never becomes "5000"
                if n.fract() == 0.0 { format!("{:.1}", n) } else { n.to_string() }
            }
            Value::Bool(b)   => b.to_string(),
            Value::Text(s)   => s.clone(),
            Value::Null      => String::new(),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_raw_string()).collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Record(fields) => {
                let inner: Vec<String> = fields.iter()
                    .map(|(k, v)| format!("{}={}", k, v.to_raw_string()))
                    .collect();
                format!("[{}]", inner.join(", "))
            }
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Value::Int(n)   => *n == 0,
            Value::Float(f) => *f == 0.0,
            _               => false,
        }
    }

    /// Ordering comparison — None if types are incomparable.
    pub fn cmp_to(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::Int(a),   Value::Int(b))   => Some(a.cmp(b)),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Int(a),   Value::Float(b)) => (*a as f64).partial_cmp(b),
            (Value::Float(a), Value::Int(b))   => a.partial_cmp(&(*b as f64)),
            (Value::Text(a),  Value::Text(b))  => Some(a.cmp(b)),
            (Value::Bool(a),  Value::Bool(b))  => Some(a.cmp(b)),
            _                                   => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_raw_string())
    }
}
