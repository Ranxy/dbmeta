use std::env;

use crate::db::ConnectionConfig;
use std::env::VarError;

macro_rules! init_db_test_service {
    ($db_type:ident, $func_name:ident, $default_port:expr) => {
        pub fn $func_name() -> Result<ConnectionConfig, VarError> {
            let _ = dotenvy::dotenv();
            let host = env::var(concat!("TEST_", stringify!($db_type), "_DB_HOST"))
                .unwrap_or_else(|_| "localhost".to_string());
            let port = env::var(concat!("TEST_", stringify!($db_type), "_DB_PORT"))
                .unwrap_or_else(|_| $default_port.to_string())
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
init_db_test_service!(MYSQL, init_mysql_test_service, "3306");
#[cfg(feature = "db-postgres")]
init_db_test_service!(POSTGRES, init_pg_test_service, "5432");

#[cfg(any(feature = "db-mysql", feature = "db-tidb"))]
pub async fn init_mysql_test_schema() -> Result<(), Box<dyn std::error::Error>> {
    use sqlx::mysql::MySqlPool;
    
    let config = init_mysql_test_service()?;
    let connection_string = format!(
        "mysql://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, config.port, config.database
    );
    
    let pool = MySqlPool::connect(&connection_string).await?;
    
    // Read and execute the main schema SQL
    let sql_content = include_str!("../../tests/fixtures/mysql_schema.sql");
    for statement in sql_content.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() && !stmt.starts_with("--") {
            sqlx::query(stmt).execute(&pool).await?;
        }
    }
    
    // Read and execute routines (procedures and functions)
    let routines_content = include_str!("../../tests/fixtures/mysql_routines.sql");
    for statement in routines_content.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() 
            && !stmt.starts_with("--") 
            && !stmt.starts_with("DROP") {
            sqlx::query(stmt).execute(&pool).await?;
        }
    }
    
    pool.close().await;
    Ok(())
}

#[cfg(feature = "db-postgres")]
pub async fn init_postgres_test_schema() -> Result<(), Box<dyn std::error::Error>> {
    use sqlx::postgres::PgPool;
    
    let config = init_pg_test_service()?;
    let connection_string = format!(
        "postgresql://{}:{}@{}:{}/{}",
        config.username, config.password, config.host, config.port, config.database
    );
    
    let pool = PgPool::connect(&connection_string).await?;
    
    // Read the SQL fixture file
    let sql_content = include_str!("../../tests/fixtures/postgres_schema.sql");
    
    // Execute the entire script as one transaction
    sqlx::query(sql_content).execute(&pool).await?;
    
    pool.close().await;
    Ok(())
}
