use super::error::DBError;

// TableKey is the map key for table metadata.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub(crate) struct TableKey {
    // Schema is the schema of a table.
    pub schema: String,
    // Table is the name of a table.
    pub table: String,
}

pub(crate) fn convert_yes_no(s: &str) -> Result<bool, DBError> {
    match s {
        "YES" | "Y" | "1" => Ok(true),
        "NO" | "N" | "0" => Ok(false),
        _ => Err(DBError::Unknow(format!("unrecognized isNullable type {s}"))),
    }
}
