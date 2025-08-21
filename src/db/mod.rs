use self::error::DBError;
use async_trait::async_trait;
use std::fmt::Debug;

mod error;
#[cfg(any(feature = "db-mysql", feature = "db-tidb"))]
mod mysql;
#[cfg(feature = "db-postgres")]
mod postgres;
pub mod store;
mod util;

#[derive(Clone, PartialEq, Debug)]
pub enum Engine {
    #[cfg(feature = "db-mysql")]
    MYSQL,
    #[cfg(feature = "db-tidb")]
    TIDB,
    #[cfg(feature = "db-postgres")]
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

#[async_trait]
pub trait DB: Send + Sync + Debug + Unpin + 'static {
    fn get_engine(&self) -> Engine;
    async fn sync_instance(&self) -> Result<store::InstanceMetadata, DBError>;
    async fn sync_database(&self) -> Result<store::DatabaseSchemaMetadata, DBError>;
}

pub async fn create_driver(cfg: &ConnectionConfig) -> Result<Box<dyn DB>, DBError> {
    match cfg.engine {
        #[cfg(feature = "db-mysql")]
        Engine::MYSQL => Ok(Box::new(mysql::Driver::create(cfg).await?)),
        #[cfg(feature = "db-tidb")]
        Engine::TIDB => Ok(Box::new(mysql::Driver::create(cfg).await?)),
        #[cfg(feature = "db-postgres")]
        Engine::POSTGRES => Ok(Box::new(postgres::Driver::create(cfg).await?)),
    }
}
