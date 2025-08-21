use std::env;

use crate::db::ConnectionConfig;
use std::env::VarError;

macro_rules! init_db_test_service {
    ($db_type:ident, $func_name:ident) => {
        pub fn $func_name() -> Result<ConnectionConfig, VarError> {
            let _ = dotenvy::dotenv();
            let host = env::var(concat!("TEST_", stringify!($db_type), "_DB_HOST"))
                .unwrap_or_else(|_| "localhost".to_string());
            let port = env::var(concat!("TEST_", stringify!($db_type), "_DB_PORT"))
                .unwrap_or_else(|_| "3306".to_string())
                .parse::<u16>()
                .map_err(|_| VarError::NotPresent)?;
            let username = env::var(concat!("TEST_", stringify!($db_type), "_DB_USERNAME"))
                .unwrap_or_default();
            let password = env::var(concat!("TEST_", stringify!($db_type), "_DB_PASSWORD"))
                .unwrap_or_default();
            let database = env::var(concat!("TEST_", stringify!($db_type), "_DB_DATABASE"))
                .unwrap_or_default();
            Ok(ConnectionConfig {
                engine: crate::db::Engine::$db_type,
                host,
                port,
                username,
                password,
                database,
            })
        }
    };
}
#[cfg(any(feature = "db-mysql", feature = "db-tidb"))]
init_db_test_service!(MYSQL, init_mysql_test_service);
#[cfg(feature = "db-postgres")]
init_db_test_service!(POSTGRES, init_pg_test_service);
