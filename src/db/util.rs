use super::error::DBError;

pub fn convert_yes_no(s: &str) -> Result<bool, DBError> {
    match s {
        "YES" | "Y" | "1" => Ok(true),
        "NO" | "N" | "0" => Ok(false),
        _ => Err(DBError::Unknow(format!(
            "unrecognized isNullable type {}",
            s
        ))),
    }
}
