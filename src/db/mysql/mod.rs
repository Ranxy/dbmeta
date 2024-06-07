use crate::db;
use sqlx::{mysql::MySqlPool, Column, Pool, Row};
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

macro_rules! create_get_function_procedure_stmt {
    ($func_name:ident, $column_name:expr) => {
        async fn $func_name(
            &self,
            database_name: &str,
            function_name: &str,
        ) -> Result<String, DBError> {
            let query = format!(
                "SHOW {} `{}`.`{}`",
                $column_name, database_name, function_name
            );
            let row = sqlx::query(&query).fetch_one(&self.pool).await?;

            let idx = if let Some(idx) = row
                .columns()
                .iter()
                .position(|column| column.name().eq_ignore_ascii_case($column_name))
            {
                Ok(idx)
            } else {
                Err(DBError::Unknow(format!("Not Find {} Failed", $column_name)))
            }?;

            let define: String = row.get(idx);

            Ok(define)
        }
    };
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

    async fn get_variable(&self, var_name: String) -> Result<String, DBError> {
        #[derive(sqlx::FromRow)]
        struct Variable {
            #[sqlx(rename = "Variable_name")]
            variable_name: String,
            #[sqlx(rename = "Value")]
            value: String,
        }

        let variable =
            sqlx::query_as::<_, Variable>(&format!("SHOW VARIABLES LIKE '{}'", var_name))
                .fetch_one(&self.pool)
                .await?;

        Ok(variable.value)
    }

    async fn get_database_info(&self, database_name: &str) -> Result<(String, String), DBError> {
        let query = "
        SELECT
			DEFAULT_CHARACTER_SET_NAME,
			DEFAULT_COLLATION_NAME
		FROM information_schema.SCHEMATA
		WHERE SCHEMA_NAME = ?
        ";

        let row = sqlx::query(query)
            .bind(database_name)
            .fetch_one(&self.pool)
            .await?;

        let character_name: String = row.get(0);
        let collation: String = row.get(1);

        Ok((character_name, collation))
    }

    async fn load_database(&self) -> Result<Vec<db::store::DatabaseSchemaMetadata>, DBError> {
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

    async fn load_column(
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

        let list = sqlx::query(query)
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
                nullable,
                r#type: column_type,
                character_set: character_set_name,
                collation,
                comment,
            };
            set_column_metadata_default(&mut col, default, nullable, &extra);

            column_map.entry(table_name).or_default().push(col);
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

        let query = if version.le(&version8_0_13) || rest.contains("MariaDB") {
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

        let list = sqlx::query(query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;

        let mut index_map = HashMap::<String, HashMap<String, db::store::IndexMetadata>>::new();

        for row in list {
            let table_name: String = row.get(0);
            let index_name: String = row.get(1);
            let column_name: Option<String> = row.get(2);
            let sub_part: i64 = row.get(3);
            let expression_name: Option<String> = row.get(4);
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

            let table_map = index_map.entry(table_name).or_default();
            let vidx = table_map.entry(index_name).or_insert(idx);

            let expression = if let Some(column_name) = column_name {
                column_name
            } else if let Some(expression_name) = expression_name {
                format!("({})", expression_name)
            } else {
                "".to_string()
            };

            vidx.expressions.push(expression);
            vidx.key_length.push(sub_part);
        }

        Ok(index_map)
    }

    async fn get_foreign_key_list(
        &self,
        database_name: &str,
    ) -> Result<HashMap<String, Vec<db::store::ForeignKeyMetadata>>, DBError> {
        let query = "
        SELECT
            fks.TABLE_NAME,
            fks.CONSTRAINT_NAME,
            kcu.COLUMN_NAME,
            fks.REFERENCED_TABLE_NAME,
            kcu.REFERENCED_COLUMN_NAME,
            fks.DELETE_RULE,
            fks.UPDATE_RULE,
            fks.MATCH_OPTION
        FROM INFORMATION_SCHEMA.REFERENTIAL_CONSTRAINTS fks
            JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu
            ON fks.CONSTRAINT_SCHEMA = kcu.TABLE_SCHEMA
                AND fks.TABLE_NAME = kcu.TABLE_NAME
                AND fks.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME
        WHERE kcu.POSITION_IN_UNIQUE_CONSTRAINT IS NOT NULL AND LOWER(fks.CONSTRAINT_SCHEMA) = ?
        ORDER BY fks.TABLE_NAME, fks.CONSTRAINT_NAME, kcu.ORDINAL_POSITION;
        ";

        let mut fk_map = HashMap::<String, Vec<db::store::ForeignKeyMetadata>>::new();

        let list = sqlx::query(query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;

        let mut build_table = String::default();
        let mut build_fk: Option<db::store::ForeignKeyMetadata> = None;

        for row in list {
            let table_name: String = row.get(0);
            let fk_name: String = row.get(1);
            let col_name: String = row.get(2);
            let ref_table: String = row.get(3);
            let ref_col: String = row.get(4);
            let on_delete: String = row.get(5);
            let on_update: String = row.get(6);
            let match_type: String = row.get(7);

            let fk = db::store::ForeignKeyMetadata {
                name: fk_name,
                columns: vec![col_name.clone()],
                referenced_schema: String::default(),
                referenced_table: ref_table,
                referenced_columns: vec![ref_col.clone()],
                on_delete,
                on_update,
                match_type,
            };

            match build_fk {
                Some(ref mut bfk) => {
                    if table_name == build_table && bfk.name == fk.name {
                        bfk.columns.push(col_name);
                        bfk.referenced_columns.push(ref_col);
                    } else {
                        let fk_vec = fk_map.entry(build_table.clone()).or_default();
                        fk_vec.push(bfk.clone());
                        build_fk = Some(fk);
                        build_table = table_name;
                    }
                }
                None => {
                    build_table = table_name;
                    build_fk = Some(fk);
                }
            }
        }

        if let Some(bfk) = build_fk {
            let fk_vec = fk_map.entry(build_table).or_default();
            fk_vec.push(bfk);
        }

        Ok(fk_map)
    }

    async fn load_table_and_view(
        &self,
        database_name: &str,
    ) -> Result<(Vec<db::store::TableMetadata>, Vec<db::store::ViewMetadata>), DBError> {
        let mut view_map = HashMap::<String, db::store::ViewMetadata>::new();

        let mut table_vec = Vec::<db::store::TableMetadata>::new();

        let view_query = "
        SELECT
        TABLE_NAME,
        VIEW_DEFINITION
    FROM information_schema.VIEWS
    WHERE TABLE_SCHEMA = ?
        ";

        let view_list = sqlx::query(view_query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;
        for row in view_list {
            let view_name: String = row.get(0);
            let definition: String = row.get(1);

            let view = db::store::ViewMetadata {
                name: view_name.clone(),
                definition,
                comment: "".to_string(),
                dependent_columns: vec![],
            };

            view_map.insert(view_name, view);
        }

        let query = "
        SELECT
            TABLE_NAME,
            TABLE_TYPE,
            IFNULL(ENGINE, ''),
            IFNULL(TABLE_COLLATION, ''),
            CAST(IFNULL(TABLE_ROWS, 0) as SIGNED),
            CAST(IFNULL(DATA_LENGTH, 0) as SIGNED),
            CAST(IFNULL(INDEX_LENGTH, 0) as SIGNED),
            CAST(IFNULL(DATA_FREE, 0) as SIGNED),
            IFNULL(CREATE_OPTIONS, ''),
            IFNULL(TABLE_COMMENT, '')
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = ?
        ORDER BY TABLE_NAME
        ";

        let list = sqlx::query(query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;

        for row in list {
            let table_name: String = row.get(0);
            let table_type: String = row.get(1);
            let comment: String = row.get(9);

            match table_type.as_str() {
                VIEW_TABLE_TYPE => {
                    if let Some(view) = view_map.get_mut(&table_name) {
                        view.comment = comment;
                    }
                    Ok(())
                }
                BASE_TABLE_TYPE => {
                    let engine: String = row.get(2);
                    let collation: Option<String> = row.get(3);
                    let row_count: i64 = row.get(4);
                    let data_size: i64 = row.get(5);
                    let index_size: i64 = row.get(6);
                    let data_free: i64 = row.get(7);
                    let options: String = row.get(8);

                    let table = db::store::TableMetadata {
                        name: table_name.clone(),
                        columns: vec![],
                        indexes: vec![],
                        engine,
                        collation,
                        row_count,
                        data_size,
                        index_size,
                        data_free,
                        create_options: options,
                        comment: comment.clone(),
                        foreign_keys: vec![],
                    };
                    table_vec.push(table);
                    Ok(())
                }
                _ => Err(DBError::Unknow(format!(
                    "Unexpected table_type {}",
                    table_type
                ))),
            }?;
        }

        let view_vec: Vec<db::store::ViewMetadata> = view_map.into_values().collect();

        Ok((table_vec, view_vec))
    }

    async fn load_routines(
        &self,
        database_name: &str,
    ) -> Result<
        (
            Vec<db::store::FunctionMetadata>,
            Vec<db::store::ProcedureMetadata>,
        ),
        DBError,
    > {
        let routines_query = "
        SELECT
            ROUTINE_NAME,
            ROUTINE_TYPE
        FROM
            INFORMATION_SCHEMA.ROUTINES
        WHERE ROUTINE_SCHEMA = ? AND ROUTINE_TYPE IN ('FUNCTION', 'PROCEDURE')
        ORDER BY ROUTINE_TYPE, ROUTINE_NAME;
        ";

        let mut functions = vec![];
        let mut procedures = vec![];

        let routines_list = sqlx::query(routines_query)
            .bind(database_name)
            .fetch_all(&self.pool)
            .await?;

        for row in routines_list {
            let name: String = row.get(0);
            let routine_type: String = row.get(1);

            if routine_type.eq_ignore_ascii_case("PROCEDURE") {
                let define = self.get_create_procedure_stmt(database_name, &name).await?;
                procedures.push(db::store::ProcedureMetadata {
                    name,
                    definition: define,
                })
            } else {
                let define = self.get_create_function_stmt(database_name, &name).await?;
                functions.push(db::store::FunctionMetadata {
                    name,
                    definition: define,
                })
            }
        }
        Ok((functions, procedures))
    }

    // Define a function for "Show Create Function"
    create_get_function_procedure_stmt!(get_create_function_stmt, "Create Function");

    // Define another function for "Show Create Procedure"
    create_get_function_procedure_stmt!(get_create_procedure_stmt, "Create Procedure");
}

impl super::DB for Driver {
    fn get_type() -> db::Engine {
        db::Engine::Mysql
    }

    async fn sync_instance(&self) -> Result<db::store::InstanceMetadata, DBError> {
        let (version, _) = self.get_version().await?;

        let databases = self.load_database().await?;

        let instance = db::store::InstanceMetadata {
            version,
            instance_roles: vec![],
            databases,
            last_sync: 0,
        };

        Ok(instance)
    }

    async fn sync_database(
        &self,
        database_name: &str,
    ) -> Result<db::store::DatabaseSchemaMetadata, DBError> {
        let (character_set, collation) = self.get_database_info(database_name).await?;
        let mut index = self.load_index(database_name).await?;
        let mut columns = self.load_column(database_name).await?;
        let mut foreign_keys = self.get_foreign_key_list(database_name).await?;
        let (tables, views) = self.load_table_and_view(database_name).await?;

        let tables = tables
            .into_iter()
            .map(|mut table| {
                let table_index_opt = index.remove(&table.name.to_string());
                if let Some(table_index) = table_index_opt {
                    let mut index_vec: Vec<db::store::IndexMetadata> =
                        table_index.into_values().collect();
                    index_vec.sort_by(|a, b| a.name.cmp(&b.name));
                    table.indexes = index_vec;
                }

                let table_column_opt = columns.remove(&table.name.to_string());
                if let Some(table_columns) = table_column_opt {
                    table.columns = table_columns;
                }

                let fk_opt = foreign_keys.remove(&table.name.to_string());
                if let Some(fk_list) = fk_opt {
                    table.foreign_keys = fk_list;
                }

                table
            })
            .collect();

        let (functions, procedures) = self.load_routines(database_name).await?;
        let schema = db::store::SchemaMetadata {
            name: String::default(),
            tables,
            external_tables: vec![],
            views,
            functions,
            procedures,
            materialized_views: vec![],
        };

        let dbmeta = db::store::DatabaseSchemaMetadata {
            name: database_name.to_string(),
            schemas: vec![schema],
            character_set,
            collation,
            extensions: vec![],
            datashare: false,
            service_name: String::default(),
        };

        Ok(dbmeta)
    }
}

fn parse_version(version: &str) -> Result<(String, String), DBError> {
    let regex = Regex::new(r#"^\d+\.\d+\.\d+"#).map_err(|e| DBError::Unknow(e.to_string()))?;
    if let Some(loc) = regex.find(version) {
        let start_index = loc.start();
        let end_index = loc.end();
        Ok((
            version[start_index..end_index].to_string(),
            version[end_index..].to_string(),
        ))
    } else {
        Err(DBError::Unknow(format!(
            "failed to parse version {}",
            version
        )))
    }
}

fn set_column_metadata_default(
    column: &mut db::store::ColumnMetadata,
    default_str: Option<String>,
    nullable_bool: bool,
    extra: &str,
) {
    if let Some(default_str) = default_str {
        if let Some(default_value) = parse_default_value(&default_str, extra) {
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
        if let Some(on_update) = parse_current_timestamp_on_update(extra) {
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
const BASE_TABLE_TYPE: &str = "BASE TABLE";
const VIEW_TABLE_TYPE: &str = "VIEW";

mod test {
    use crate::db::{ConnectionConfig, DB};

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

        println!("VERSION:{:?}\n", v);

        let db_metadatas = d.load_database().await.unwrap();

        println!("db:{:?}\n", db_metadatas);

        let db = d.sync_database("exp").await.unwrap();

        println!("exp:{:?}\n", db);
    }
}
