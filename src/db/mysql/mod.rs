use crate::db;
use sqlx::{mysql::MySqlPool, Pool, Row};
use std::collections::HashMap;

use super::{error::DBError, util};

use regex::Regex;
use version_compare::Version;

const SYSTEM_DB: &str = "'information_schema','mysql','performance_schema','sys'";

pub struct Driver {
    db_type: db::Engine,
    database_name: String,
    pool: Pool<sqlx::MySql>,
}

impl Driver {
    async fn create(cfg: db::ConnectionConfig) -> Result<Driver, DBError> {
        let opt = sqlx::mysql::MySqlConnectOptions::default()
            .host(&cfg.host)
            .port(cfg.port)
            .username(&cfg.username)
            .password(&cfg.password)
            .database(&cfg.connection_database)
            .ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);

        let pool = MySqlPool::connect_with(opt).await?;

        Ok(Driver {
            db_type: db::Engine::Mysql,
            database_name: cfg.database,
            pool,
        })
    }

    async fn get_version(&self) -> Result<(String, String), DBError> {
        #[derive(sqlx::FromRow)]
        struct Version {
            version: String,
        }
        let version = sqlx::query_as::<_, Version>("SELECT VERSION() as version")
            .fetch_one(&self.pool)
            .await?;
        let pversion = parse_version(&version.version)?;
        Ok(pversion)
    }

    async fn get_variable(&self, varName: String) -> Result<String, DBError> {
        #[derive(sqlx::FromRow)]
        struct Variable {
            Variable_name: String,
            Value: String,
        }

        let variable = sqlx::query_as::<_, Variable>(&format!("SHOW VARIABLES LIKE '{}'", varName))
            .fetch_one(&self.pool)
            .await?;

        Ok(variable.Value)
    }

    async fn get_database(&self) -> Result<Vec<db::store::DatabaseSchemaMetadata>, DBError> {
        let query = format!(
            "SELECT
        SCHEMA_NAME,
        DEFAULT_CHARACTER_SET_NAME,
        DEFAULT_COLLATION_NAME
        FROM information_schema.SCHEMATA
        WHERE LOWER(SCHEMA_NAME) NOT IN ({})
        ",
            SYSTEM_DB
        );

        let databases = sqlx::query(&query).fetch_all(&self.pool).await?;

        let db_metadatas = databases
            .iter()
            .map(|row| {
                let schema_name: String = row.get("SCHEMA_NAME");
                let default_character_set_name: String = row.get("DEFAULT_CHARACTER_SET_NAME");
                let default_collation_name: String = row.get("DEFAULT_COLLATION_NAME");

                db::store::DatabaseSchemaMetadata {
                    name: schema_name,
                    schemas: vec![],
                    character_set: default_character_set_name,
                    collation: default_collation_name,
                    extensions: vec![],
                    datashare: false,
                    service_name: "".to_string(),
                }
            })
            .collect();

        Ok(db_metadatas)
    }

    async fn sync_columns(
        &self,
        database_name: &str,
    ) -> Result<HashMap<String, Vec<db::store::ColumnMetadata>>, DBError> {
        let query = r"
        SELECT
            TABLE_NAME,
            IFNULL(COLUMN_NAME, '') as COLUMN_NAME,
            ORDINAL_POSITION,
            COLUMN_DEFAULT,
            IS_NULLABLE,
            COLUMN_TYPE,
            IFNULL(CHARACTER_SET_NAME, '') as CHARACTER_SET_NAME,
            IFNULL(COLLATION_NAME, '') as COLLATION_NAME,
            COLUMN_COMMENT,
            EXTRA
        FROM information_schema.COLUMNS
            WHERE TABLE_SCHEMA = ?
            ORDER BY TABLE_NAME, ORDINAL_POSITION
        ";

        let list = sqlx::query(&query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;

        let mut column_map = HashMap::<String, Vec<db::store::ColumnMetadata>>::new();

        for row in list {
            let table_name: String = row.get("TABLE_NAME");
            let column_name: String = row.get("COLUMN_NAME");
            let position: u32 = row.get("ORDINAL_POSITION");
            let default: Option<String> = row.get("COLUMN_DEFAULT");
            let nullable_str: String = row.get("IS_NULLABLE");
            let column_type: String = row.get("COLUMN_TYPE");
            let character_set_name: String = row.get("CHARACTER_SET_NAME");
            let collation: String = row.get("COLLATION_NAME");
            let comment: String = row.get("COLUMN_COMMENT");
            let extra: String = row.get("EXTRA");

            let nullable = util::convert_yes_no(&nullable_str)?;
            let mut col = db::store::ColumnMetadata {
                name: column_name,
                position: position as i32,
                default_value: None,
                on_update: None,
                nullable: nullable,
                r#type: column_type,
                character_set: character_set_name,
                collation: collation,
                comment: comment,
            };
            set_column_metadata_default(&mut col, default, nullable, &extra);

            column_map.entry(table_name).or_insert(Vec::new()).push(col);
        }

        Ok(column_map)
    }

    async fn load_index(
        &self,
        database_name: &str,
    ) -> Result<HashMap<String, HashMap<String, db::store::IndexMetadata>>, DBError> {
        let (version_str, rest) = self.get_version().await?;

        let version = Version::from(&version_str).ok_or(DBError::Unknow(format!(
            "db version {} cannot be parsed",
            version_str
        )))?;

        let version8_0_13 = Version::from("8.0.13").unwrap();

        let query = if version.ge(&version8_0_13) || rest.contains("MariaDB") {
            "
            SELECT
                TABLE_NAME,
                INDEX_NAME,
                COLUMN_NAME,
                IFNULL(SUB_PART, -1),
                '',
                INDEX_TYPE,
                CASE NON_UNIQUE WHEN 0 THEN 1 ELSE 0 END AS IS_UNIQUE,
                1,
                INDEX_COMMENT
            FROM information_schema.STATISTICS
            WHERE TABLE_SCHEMA = ?
            ORDER BY TABLE_NAME, INDEX_NAME, SEQ_IN_INDEX"
        } else {
            "
            SELECT
                TABLE_NAME,
                INDEX_NAME,
                COLUMN_NAME,
                IFNULL(SUB_PART, -1),
                EXPRESSION,
                INDEX_TYPE,
                CASE NON_UNIQUE WHEN 0 THEN 1 ELSE 0 END AS IS_UNIQUE,
                CASE IS_VISIBLE WHEN 'YES' THEN 1 ELSE 0 END,
                INDEX_COMMENT
            FROM information_schema.STATISTICS
            WHERE TABLE_SCHEMA = ?
            ORDER BY TABLE_NAME, INDEX_NAME, SEQ_IN_INDEX
            "
        };

        let list = sqlx::query(&query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;

        let mut index_map = HashMap::<String, HashMap<String, db::store::IndexMetadata>>::new();

        for row in list {
            let table_name: String = row.get(0);
            let index_name: String = row.get(1);
            let column_name: Option<String> = row.get(2);
            let sub_part: i64 = row.get(3);
            let expression: String = row.get(4);
            // let seq_in_index: u16 = row.get(5);
            let index_type: String = row.get(5);
            let is_unique: i16 = row.get(6);
            let is_visible: i16 = row.get(7);
            let comment: String = row.get(8);

            let is_primary = index_name == "PRIMARY";

            let idx = db::store::IndexMetadata {
                name: index_name.clone(),
                expressions: vec![],
                key_length: vec![],
                r#type: index_type,
                unique: is_unique == 1,
                primary: is_primary,
                visible: is_visible == 1,
                comment,
                definition: "".to_string(),
            };

            let mut table_map = index_map.entry(table_name).or_insert(HashMap::new());
            let vidx = table_map.entry(index_name).or_insert(idx);

            vidx.expressions.push(expression);
            vidx.key_length.push(sub_part);
        }

        Ok(index_map)
    }
}

impl super::DB for Driver {
    fn get_type() -> db::Engine {
        return db::Engine::Mysql;
    }

    async fn sync_instance(&self) -> Result<db::store::InstanceMetadata, DBError> {
        let version = self.get_version().await?;

        Err(DBError::Args("(todo)".to_string()))
    }

    async fn sync_db(&self) -> Result<db::store::DatabaseSchemaMetadata, DBError> {
        todo!()
    }
}

fn parse_version(version: &str) -> Result<(String, String), DBError> {
    let regex = Regex::new(r#"^\d+\.\d+\.\d+"#).map_err(|e| DBError::Unknow(e.to_string()))?;
    if let Some(loc) = regex.find(&version) {
        let start_index = loc.start();
        let end_index = loc.end();
        Ok((
            version[start_index..end_index].to_string(),
            version[end_index..].to_string(),
        ))
    } else {
        Err(DBError::Unknow(
            format!("failed to parse version {}", version).into(),
        ))
    }
}

fn set_column_metadata_default(
    column: &mut db::store::ColumnMetadata,
    default_str: Option<String>,
    nullable_bool: bool,
    extra: &str,
) {
    if let Some(default_str) = default_str {
        if let Some(default_value) = parse_default_value(&default_str, &extra) {
            column.default_value = Some(default_value);
        }
    } else if extra.to_uppercase().contains(AUTO_INCREMENT_SYMBOL) {
        column.default_value = Some(db::store::ColumnMetadataDefaultValue::DefaultExpression(
            AUTO_INCREMENT_SYMBOL.to_string(),
        ))
    } else if nullable_bool {
        column.default_value = Some(db::store::ColumnMetadataDefaultValue::DefaultNull(true))
    }

    if extra.contains("on update CURRENT_TIMESTAMP") {
        if let Some(on_update) = parse_current_timestamp_on_update(&extra) {
            column.on_update = Some(on_update);
        }
    }
}

fn parse_default_value(
    default_str: &str,
    extra: &str,
) -> Option<db::store::ColumnMetadataDefaultValue> {
    if is_current_timestamp_like(default_str) {
        Some(db::store::ColumnMetadataDefaultValue::DefaultExpression(
            default_str.to_string(),
        ))
    } else if extra.contains("DEFAULT_GENERATED") {
        Some(db::store::ColumnMetadataDefaultValue::DefaultExpression(
            format!("({})", default_str),
        ))
    } else {
        Some(db::store::ColumnMetadataDefaultValue::Default(
            default_str.to_string(),
        ))
    }
}

fn is_current_timestamp_like(default_str: &str) -> bool {
    // Check if the default value is similar to CURRENT_TIMESTAMP
    default_str.eq_ignore_ascii_case("CURRENT_TIMESTAMP")
        || Regex::new(r#"^CURRENT_TIMESTAMP\(\d+\)$"#)
            .unwrap()
            .is_match(default_str)
}

fn parse_current_timestamp_on_update(extra: &str) -> Option<String> {
    if let Some(digits) = extract_digits_from_current_timestamp(extra) {
        Some(format!("CURRENT_TIMESTAMP({})", digits))
    } else {
        Some("CURRENT_TIMESTAMP".to_string())
    }
}

fn extract_digits_from_current_timestamp(extra: &str) -> Option<&str> {
    let re = Regex::new(r#"CURRENT_TIMESTAMP\((\d+)\)"#).unwrap();
    if let Some(captures) = re.captures(extra) {
        captures.get(1).map(|m| m.as_str())
    } else {
        None
    }
}

const AUTO_INCREMENT_SYMBOL: &str = "AUTO_INCREMENT";

mod test {
    use crate::db::ConnectionConfig;

    use super::Driver;

    #[tokio::test]
    async fn test_get_version() {
        let d = Driver::create(ConnectionConfig {
            host: "127.0.0.1".to_string(),
            port: 3306,
            username: "root".to_string(),
            password: "rootadmin".to_string(),
            database: "exp".to_string(),
            connection_database: "exp".to_string(),
        })
        .await
        .unwrap();

        let v = d.get_version().await.unwrap();

        println!("VERSION:{:?}", v);

        let db_metadatas = d.get_database().await.unwrap();

        println!("db:{:?}", db_metadatas);

        let cols = d.sync_columns("exp").await.unwrap();

        println!("cols:{:?}", cols);

        let idxs = d.load_index("exp").await.unwrap();

        println!("idxs:{:?}", idxs);
    }
}
