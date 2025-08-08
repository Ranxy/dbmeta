use self::error::DBError;
use async_trait::async_trait;
use std::fmt::Debug;

mod error;
mod mysql;
mod postgres;
pub mod store;
mod util;

#[derive(Clone, PartialEq, Debug)]
pub enum Engine {
    MYSQL,
    TIDB,
    POSTGRES,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ConnectionConfig {
    pub engine: Engine,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
}

// TableKey is the map key for table metadata.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
struct TableKey {
    // Schema is the schema of a table.
    pub schema: String,
    // Table is the name of a table.
    pub table: String,
}

#[async_trait]
pub trait DB: Send + Sync + Debug + Unpin + 'static {
    // fn get_type(&self) -> Engine;
    async fn sync_instance(&self) -> Result<store::InstanceMetadata, DBError>;
    async fn sync_database(
        &self,
        database_name: &str,
    ) -> Result<store::DatabaseSchemaMetadata, DBError>;
}

pub async fn create_driver(engine: Engine, cfg: &ConnectionConfig) -> Result<Box<dyn DB>, DBError> {
    match engine {
        Engine::MYSQL => Ok(Box::new(mysql::Driver::create(cfg).await?)),
        Engine::TIDB => Ok(Box::new(mysql::Driver::create(cfg).await?)),
        Engine::POSTGRES => Ok(Box::new(postgres::Driver::create(cfg).await?)),
    }
}
