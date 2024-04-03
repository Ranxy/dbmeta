use self::error::DBError;
use url::Url;

mod error;
mod mysql;
pub mod store;
mod util;

pub enum Engine {
    Mysql,
    Tidb,
    Postgres,
}

#[derive(Clone, PartialEq)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub connection_database: String,
}

pub trait DB {
    fn get_type() -> Engine;
    async fn sync_instance(&self) -> Result<store::InstanceMetadata, DBError>;
    async fn sync_db(&self) -> Result<store::DatabaseSchemaMetadata, DBError>;
}
