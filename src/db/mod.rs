use std::{future::Future, pin::Pin};

use self::error::DBError;

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
    async fn sync_database(&self, database_name: &str) -> Result<store::DatabaseSchemaMetadata, DBError>;
}
