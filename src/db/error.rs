#[derive(Debug)]
pub enum DBError {
    Todo,
    Args(String),
    DB(sqlx::Error),
    Unknow(String),
}

impl From<sqlx::Error> for DBError {
    fn from(value: sqlx::Error) -> Self {
        DBError::DB(value)
    }
}
impl From<url::ParseError> for DBError {
    fn from(value: url::ParseError) -> Self {
        DBError::Args(value.to_string())
    }
}
