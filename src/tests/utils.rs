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
    let config = init_mysql_test_service()?;

    // Use the mysql command line client to execute the schema file
    let sql_file_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mysql_schema.sql");

    // Use MYSQL_PWD environment variable instead of command line argument for security
    let status = std::process::Command::new("mysql")
        .env("MYSQL_PWD", &config.password)
        .arg("--protocol=TCP")
        .arg(format!("--host={}", config.host))
        .arg(format!("--port={}", config.port))
        .arg(format!("--user={}", config.username))
        .arg(&config.database)
        .stdin(std::process::Stdio::from(std::fs::File::open(
            sql_file_path,
        )?))
        .status()?;

    if !status.success() {
        return Err(format!(
            "Failed to execute MySQL schema: exit code {:?}",
            status.code()
        )
        .into());
    }

    // Execute the routines file (stored procedures and functions)
    let routines_file_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mysql_routines.sql");

    // Use --delimiter to handle procedure/function definitions
    let routines_status = std::process::Command::new("mysql")
        .env("MYSQL_PWD", &config.password)
        .arg("--protocol=TCP")
        .arg(format!("--host={}", config.host))
        .arg(format!("--port={}", config.port))
        .arg(format!("--user={}", config.username))
        .arg(&config.database)
        .stdin(std::process::Stdio::from(std::fs::File::open(
            routines_file_path,
        )?))
        .status()?;

    if !routines_status.success() {
        return Err(format!(
            "Failed to execute MySQL routines: exit code {:?}",
            routines_status.code()
        )
        .into());
    }

    Ok(())
}

#[cfg(feature = "db-postgres")]
pub async fn init_postgres_test_schema() -> Result<(), Box<dyn std::error::Error>> {
    let config = init_pg_test_service()?;

    // Use the psql command line client to execute the schema file
    let sql_file_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/postgres_schema.sql");

    // Set environment variable for password
    let status = std::process::Command::new("psql")
        .env("PGPASSWORD", &config.password)
        .arg(format!("--host={}", config.host))
        .arg(format!("--port={}", config.port))
        .arg(format!("--username={}", config.username))
        .arg(format!("--dbname={}", config.database))
        .arg("--file")
        .arg(sql_file_path)
        .status()?;

    if !status.success() {
        return Err(format!(
            "Failed to execute PostgreSQL schema: exit code {:?}",
            status.code()
        )
        .into());
    }

    Ok(())
}
