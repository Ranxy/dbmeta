use crate::db;
use crate::db::postgres::system;
use crate::db::{error::DBError, util};

use sqlx::{PgPool, Pool, Postgres, Row};
use std::collections::HashMap;

use async_trait::async_trait;
use std::fmt::Debug;
use std::fmt::Formatter;

use regex::Regex;

pub struct Driver {
    engine: db::Engine,
    database_name: String,
    pool: Pool<Postgres>,
}

impl Debug for Driver {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Driver");
        ds.field("engine", &self.engine);
        ds.field("database_name", &self.database_name);
        ds.finish()
    }
}

#[async_trait]
impl db::DB for Driver {
    fn get_engine(&self) -> db::Engine {
        self.engine.clone()
    }

    async fn sync_instance(&self) -> Result<db::store::InstanceMetadata, DBError> {
        let version = self.get_version().await?;
        let databases = self.load_database().await?;

        Ok(db::store::InstanceMetadata {
            version,
            instance_roles: vec![], // TODO: Implement roles if needed
            databases: databases
                .into_iter()
                .filter(|db| !system::SYSTEM_DATABASES.contains(db.name.as_str()))
                .collect(),
            last_sync: 0,
        })
    }

    async fn sync_database(&self) -> Result<db::store::DatabaseSchemaMetadata, DBError> {
        let databases = self.load_database().await?;
        let mut database = databases
            .into_iter()
            .find(|db| db.name == self.database_name)
            .ok_or_else(|| DBError::Args(format!("Database '{}' not found", self.database_name)))?;

        let txn = self.pool.begin().await?;

        let schemas = self.load_schema().await?;
        let columns = self.load_column().await?;
        let indexs = self.load_index().await?;
        let tables = self.load_table(&columns, &indexs).await?;
        let views = self.load_view().await?;
        let mat_views = self.get_materialized_view().await?;

        for schema in schemas {
            let schema_name = schema.name.clone();
            let tables_in_schema = tables.get(&schema_name).cloned().unwrap_or_default();
            let views_in_schema = views.get(&schema_name).cloned().unwrap_or_default();
            let mat_views_in_schema = mat_views.get(&schema_name).cloned().unwrap_or_default();

            let schema_metadata = db::store::SchemaMetadata {
                name: schema.name,
                tables: tables_in_schema,
                external_tables: vec![], // TODO: Implement external tables if needed
                views: views_in_schema,
                functions: vec![], // TODO: Implement functions if needed
                materialized_views: mat_views_in_schema,
                procedures: vec![],
                owner: schema.owner,
                comment: schema.comment,
            };

            database.schemas.push(schema_metadata);
        }

        txn.commit().await?;

        Ok(database)
    }
}

#[derive(Debug, Clone)]
struct SchemaInfo {
    name: String,
    owner: String,
    comment: String,
}

impl Driver {
    pub async fn create(cfg: &db::ConnectionConfig) -> Result<impl db::DB, DBError> {
        return Self::create_driver(cfg).await;
    }

    pub async fn create_driver(cfg: &db::ConnectionConfig) -> Result<Driver, DBError> {
        let opt = sqlx::postgres::PgConnectOptions::default()
            .host(&cfg.host)
            .port(cfg.port)
            .username(&cfg.username)
            .password(&cfg.password)
            .database(&cfg.database);

        let pool = PgPool::connect_with(opt).await?;

        Ok(Driver {
            engine: cfg.engine.clone(),
            database_name: cfg.database.clone(),
            pool,
        })
    }

    async fn get_version(&self) -> Result<String, DBError> {
        let version: String = sqlx::query("SHOW server_version_num")
            .fetch_one(&self.pool)
            .await?
            .get(0);

        let nv: i64 = version
            .parse()
            .map_err(|e| DBError::Unknow(format!("PG VERSION ERROR:{e}")))?;
        let (marjor, minor, patch) = (nv / 10000, (nv / 100) % 100, nv % 100);
        Ok(format!("{marjor}.{minor}.{patch}"))
    }

    async fn load_database(&self) -> Result<Vec<db::store::DatabaseSchemaMetadata>, DBError> {
        let query = "
    SELECT datname, 
        pg_encoding_to_char(encoding) as character_set, 
        datcollate, 
        pg_catalog.pg_get_userbyid(datdba) as db_owner 
    FROM pg_database;
        ";

        let databases = sqlx::query(query).fetch_all(&self.pool).await?;

        let db_metadatas = databases
            .iter()
            .map(|row| {
                let name: String = row.get("datname");
                let character_set: String = row.get("character_set");
                let collation: String = row.get("datcollate");
                let owner: String = row.get("db_owner");

                db::store::DatabaseSchemaMetadata {
                    name,
                    schemas: vec![],
                    character_set,
                    collation,
                    extensions: vec![],
                    datashare: false,
                    service_name: String::new(),
                    owner,
                }
            })
            .collect();

        Ok(db_metadatas)
    }

    async fn load_schema(&self) -> Result<Vec<SchemaInfo>, DBError> {
        let query = format!(
            "
    SELECT nspname, pg_catalog.pg_get_userbyid(nspowner) as schema_owner, 
        obj_description(oid, 'pg_namespace') as schema_comment
    FROM pg_catalog.pg_namespace
    WHERE nspname NOT IN ({})
    ORDER BY nspname;
        ",
            *system::SYSTEM_SCHEMAS_STRING
        );

        let list = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut schema_vec = Vec::<SchemaInfo>::new();
        for row in list {
            let schemaname: String = row.get("nspname");
            let owner: String = row.get("schema_owner");
            let comment: Option<String> = row.get("schema_comment");

            let si = SchemaInfo {
                name: schemaname,
                owner,
                comment: comment.unwrap_or_default(),
            };

            schema_vec.push(si);
        }

        Ok(schema_vec)
    }

    async fn load_column(
        &self,
    ) -> Result<HashMap<util::TableKey, Vec<db::store::ColumnMetadata>>, DBError> {
        let query = format!(
            r"
    SELECT
        cols.table_schema,
        cols.table_name,
        cols.column_name,
        cols.data_type,
        cols.character_maximum_length,
        cols.ordinal_position,
        cols.column_default,
        cols.is_nullable,
        cols.collation_name,
        cols.udt_schema,
        cols.udt_name,
        cols.identity_generation,
        pg_catalog.col_description(format('%s.%s', quote_ident(table_schema), quote_ident(table_name))::regclass, cols.ordinal_position::int) as column_comment
    FROM INFORMATION_SCHEMA.COLUMNS AS cols
    WHERE cols.table_schema NOT IN ({})
    ORDER BY cols.table_schema, cols.table_name, cols.ordinal_position;
        ",
            *system::SYSTEM_SCHEMAS_STRING
        );

        let list = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut column_map = HashMap::<util::TableKey, Vec<db::store::ColumnMetadata>>::new();

        for row in list {
            let schema_name: String = row.get("table_schema");
            let table_name: String = row.get("table_name");
            let column_name: String = row.get("column_name");
            let data_type: String = row.get("data_type");
            let character_maximum_length: Option<i32> = row.get("character_maximum_length");
            let position: i32 = row.get("ordinal_position");
            let default: Option<String> = row.get("column_default");
            let nullable_str: String = row.get("is_nullable");
            let collation: Option<String> = row.get("collation_name");
            let udt_schema: Option<String> = row.get("udt_schema");
            let udt_name: Option<String> = row.get("udt_name");
            let identity_generation: Option<String> = row.get("identity_generation");
            let comment: Option<String> = row.get("column_comment");

            let r#type = match data_type.as_str() {
                "USER-DEFINED" => {
                    format!(
                        "{}.{}",
                        udt_schema.unwrap_or_default(),
                        udt_name.unwrap_or_default()
                    )
                }
                "ARRAY" => udt_name.unwrap_or_default().to_string(),
                "character" | "character varying" | "bit" | "bit varying" => {
                    if let Some(length) = character_maximum_length {
                        format!("{data_type}({length})")
                    } else {
                        data_type.clone()
                    }
                }
                _ => data_type.clone(),
            };

            let col = db::store::ColumnMetadata {
                name: column_name,
                position,
                default: default.unwrap_or_default(),
                on_update: None,
                nullable: util::convert_yes_no(&nullable_str)?,
                r#type,
                character_set: String::new(), // Postgres does not have character set
                collation: collation.unwrap_or_default(),
                comment: comment.unwrap_or_default(),
                identity_generation: match identity_generation.as_deref() {
                    Some("ALWAYS") => db::store::IdentityGeneration::Always,
                    Some("BY DEFAULT") => db::store::IdentityGeneration::ByDefault,
                    _ => db::store::IdentityGeneration::UNSPECIFIED,
                },
            };
            column_map
                .entry(util::TableKey {
                    schema: schema_name,
                    table: table_name,
                })
                .or_default()
                .push(col);
        }

        Ok(column_map)
    }

    async fn load_index(
        &self,
    ) -> Result<HashMap<util::TableKey, Vec<db::store::IndexMetadata>>, DBError> {
        let query = format!(
            r"
    SELECT idx.schemaname, idx.tablename, idx.indexname, idx.indexdef, (SELECT 1
        FROM information_schema.table_constraints
        WHERE constraint_schema = idx.schemaname
        AND constraint_name = idx.indexname
        AND table_schema = idx.schemaname
        AND table_name = idx.tablename
        AND constraint_type = 'PRIMARY KEY') AS primary,
        obj_description(format('%s.%s', quote_ident(idx.schemaname), quote_ident(idx.indexname))::regclass) AS comment
    FROM pg_indexes AS idx WHERE idx.schemaname NOT IN ({})
    ORDER BY idx.schemaname, idx.tablename, idx.indexname;
        ",
            *system::SYSTEM_SCHEMAS_STRING
        );

        let list = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut index_map = HashMap::<util::TableKey, Vec<db::store::IndexMetadata>>::new();

        for row in list {
            let schema_name: String = row.get("schemaname");
            let table_name: String = row.get("tablename");
            let index_name: String = row.get("indexname");
            let index_def: String = row.get("indexdef");
            let is_primary: Option<i32> = row.get("primary");
            let comment: Option<String> = row.get("comment");

            let idx = db::store::IndexMetadata {
                name: index_name.clone(),
                expressions: vec![],
                key_length: vec![],
                r#type: get_index_method_type(&index_def).unwrap_or_default(),
                unique: false, //TODO: need to parse this from index_def
                primary: is_primary.map(|v| v == 1).unwrap_or(false),
                visible: true,
                comment: comment.unwrap_or_default(),
                definition: index_def.clone(),
            };

            let key = util::TableKey {
                schema: schema_name,
                table: table_name,
            };

            index_map.entry(key).or_default().push(idx);
        }

        Ok(index_map)
    }

    async fn load_table(
        &self,
        column_map: &HashMap<util::TableKey, Vec<db::store::ColumnMetadata>>,
        index_map: &HashMap<util::TableKey, Vec<db::store::IndexMetadata>>,
    ) -> Result<HashMap<String, Vec<db::store::TableMetadata>>, DBError> {
        let query = format!(
            r"
    SELECT tbl.schemaname, tbl.tablename,
        pg_table_size(format('%s.%s', quote_ident(tbl.schemaname), quote_ident(tbl.tablename))::regclass) AS data_size,
        pg_indexes_size(format('%s.%s', quote_ident(tbl.schemaname), quote_ident(tbl.tablename))::regclass) AS index_size,
        GREATEST(pc.reltuples::bigint, 0::BIGINT) AS estimate,
        obj_description(format('%s.%s', quote_ident(tbl.schemaname), quote_ident(tbl.tablename))::regclass) AS comment,
        tbl.tableowner
    FROM pg_catalog.pg_tables tbl
    LEFT JOIN pg_class as pc ON pc.oid = format('%s.%s', quote_ident(tbl.schemaname), quote_ident(tbl.tablename))::regclass
    WHERE tbl.schemaname NOT IN ({})
    ORDER BY tbl.schemaname, tbl.tablename;
            ",
            *system::SYSTEM_SCHEMAS_STRING
        );

        let list = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut table_map = HashMap::<String, Vec<db::store::TableMetadata>>::new();
        for row in list {
            let schema_name: String = row.get("schemaname");
            let table_name: String = row.get("tablename");
            let data_size: i64 = row.get("data_size");
            let index_size: i64 = row.get("index_size");
            let row_count: i64 = row.get("estimate");
            let comment: Option<String> = row.get("comment");
            let owner: String = row.get("tableowner");

            let key = util::TableKey {
                schema: schema_name.clone(),
                table: table_name.clone(),
            };

            let columns = column_map.get(&key).cloned().unwrap_or_default();
            let indexes = index_map.get(&key).cloned().unwrap_or_default();

            let table_metadata = db::store::TableMetadata {
                name: table_name,
                columns,
                indexes,
                engine: String::new(),
                collation: None,
                row_count,
                data_size,
                index_size,
                data_free: 0,
                create_options: String::new(), // Postgres does not have create options like MySQL
                comment: comment.unwrap_or_default(),
                owner,
                foreign_keys: vec![],
            };

            table_map
                .entry(schema_name)
                .or_default()
                .push(table_metadata);
        }

        Ok(table_map)
    }

    async fn load_view(&self) -> Result<HashMap<String, Vec<db::store::ViewMetadata>>, DBError> {
        let query = format!(
            r"
    SELECT pc.oid, schemaname, viewname, definition, obj_description(format('%s.%s', quote_ident(schemaname), quote_ident(viewname))::regclass) as comment
    FROM pg_catalog.pg_views
        LEFT JOIN pg_class as pc ON pc.oid = format('%s.%s', quote_ident(schemaname), quote_ident(viewname))::regclass
    WHERE schemaname NOT IN ({})
    ORDER BY schemaname, viewname;
        ",
            *system::SYSTEM_SCHEMAS_STRING
        );

        let list = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut view_map = HashMap::<String, Vec<db::store::ViewMetadata>>::new();

        for row in list {
            let schema_name: String = row.get("schemaname");
            let view_name: String = row.get("viewname");
            let definition: String = row.get("definition");
            let comment: Option<String> = row.get("comment");

            let view_metadata = db::store::ViewMetadata {
                name: view_name,
                definition,
                comment: comment.unwrap_or_default(),
                dependent_columns: vec![], //TODO we can implement this later
            };

            view_map.entry(schema_name).or_default().push(view_metadata);
        }

        Ok(view_map)
    }

    async fn get_materialized_view(
        &self,
    ) -> Result<HashMap<String, Vec<db::store::MaterializedViewMetadata>>, DBError> {
        let query = format!(
            r"
    SELECT pc.oid, schemaname, matviewname, definition, obj_description(format('%s.%s', quote_ident(schemaname), quote_ident(matviewname))::regclass) as comment
    FROM pg_catalog.pg_matviews
        LEFT JOIN pg_class as pc ON pc.oid = format('%s.%s', quote_ident(schemaname), quote_ident(matviewname))::regclass
    WHERE schemaname NOT IN ({})
    ORDER BY schemaname, matviewname;
            ",
            *system::SYSTEM_SCHEMAS_STRING
        );
        let list = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut matview_map = HashMap::<String, Vec<db::store::MaterializedViewMetadata>>::new();

        for row in list {
            let schema_name: String = row.get("schemaname");
            let matview_name: String = row.get("matviewname");
            let definition: String = row.get("definition");
            let comment: Option<String> = row.get("comment");

            let matview_metadata = db::store::MaterializedViewMetadata {
                name: matview_name,
                definition,
                comment: comment.unwrap_or_default(),
                dependent_columns: vec![], //TODO we can implement this later
            };

            matview_map
                .entry(schema_name)
                .or_default()
                .push(matview_metadata);
        }

        Ok(matview_map)
    }
}

fn get_index_method_type(stmt: &str) -> Option<String> {
    let re = Regex::new(r"USING (\w+) ").unwrap();
    re.captures(stmt)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod test {

    use crate::db::DB;

    use crate::tests::init_pg_test_service;

    use super::Driver;

    async fn get_driver() -> Driver {
        let cfg = init_pg_test_service().unwrap();
        Driver::create_driver(&cfg).await.unwrap()
    }

    #[tokio::test]
    async fn test_schema() {
        let d = get_driver().await;
        let v = d.get_version().await.unwrap();
        println!("Postgres version: {}", v);

        let databases = d.load_database().await.unwrap();
        println!("Databases: {:?}", databases);

        let schemas = d.load_schema().await.unwrap();
        println!("Schemas: {:?}", schemas);
    }

    #[tokio::test]
    async fn test_table() {
        let d = get_driver().await;
        let column_map = d.load_column().await.unwrap();
        println!("Columns: {:?} \n", column_map);

        let index_map = d.load_index().await.unwrap();
        println!("Indexes: {:?} \n", index_map);

        let table_map = d.load_table(&column_map, &index_map).await.unwrap();
        println!("Tables: {:?} \n", table_map);

        let view_map = d.load_view().await.unwrap();
        println!("Views: {:?} \n", view_map);

        let mat_view_map = d.get_materialized_view().await.unwrap();
        println!("Materialized Views: {:?} \n", mat_view_map);
    }

    #[tokio::test]
    async fn test_db() {
        let d = get_driver().await;
        let ins = d.sync_instance().await.unwrap();
        println!("Instance Metadata: {:?}", ins);

        let s = d.sync_database().await.unwrap();
        println!("Database Metadata: {:?}", s);
    }
}
