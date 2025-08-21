#[derive(Debug)]
pub enum DBError {
    Todo,
    Args(String),
    DB(String),
    Unknow(String),
}
#[cfg(any(feature = "db-mysql", feature = "db-tidb",feature="db-postgres"))]
impl From<sqlx::Error> for DBError {
    fn from(value: sqlx::Error) -> Self {
        DBError::DB(value.to_string())
    }
}
impl From<url::ParseError> for DBError {
    fn from(value: url::ParseError) -> Self {
        DBError::Args(value.to_string())
    }
}
