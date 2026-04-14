pub mod table;
pub mod pipeline;

pub use table::{Column, Table, RawWorkbook};
pub use pipeline::{build_table, build_table_from_json};
