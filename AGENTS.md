# dbmeta - Project Architecture and Components

## Project Overview

**dbmeta** is an open-source Rust library that provides a unified interface for managing and interacting with multiple database backends. It offers a consistent API for obtaining database metadata across different database systems including MySQL, TiDB, and PostgreSQL.

### Key Features

- **Multi-database Support**: Unified interface for MySQL, TiDB, and PostgreSQL
- **Comprehensive Metadata**: Extract detailed information about database structures
- **Async/Await**: Built on Tokio for asynchronous operations
- **Feature-based Compilation**: Include only the database drivers you need
- **Type-safe**: Leverages Rust's type system for safe database operations

## Architecture

### Core Components

#### 1. Database Engine Abstraction (`src/db/mod.rs`)

The core abstraction layer that provides a unified interface for all database operations:

```rust
pub trait DB: Send + Sync + Debug + Unpin + 'static {
    fn get_engine(&self) -> Engine;
    async fn sync_instance(&self) -> Result<store::InstanceMetadata, DBError>;
    async fn sync_database(&self) -> Result<store::DatabaseSchemaMetadata, DBError>;
}
```

**Components**:
- `Engine`: Enum defining supported database types (MYSQL, TIDB, POSTGRES)
- `ConnectionConfig`: Configuration structure for database connections
- `DB` trait: Unified interface that all database drivers implement
- `create_driver()`: Factory function to instantiate appropriate database drivers

#### 2. Metadata Store (`src/db/store.rs`)

Defines the data structures for representing database metadata:

**Instance Level**:
- `InstanceMetadata`: Database server information including version, roles, and databases
- `InstanceRoleMetadata`: User roles and permissions

**Database Level**:
- `DatabaseSchemaMetadata`: Complete database schema information
- `SchemaMetadata`: Schema-level organization (PostgreSQL concept)
- `ExtensionMetadata`: Database extensions (PostgreSQL)

**Object Level**:
- `TableMetadata`: Table structure, columns, indexes, and statistics
- `ColumnMetadata`: Column definitions with types, constraints, and defaults
- `IndexMetadata`: Index definitions and properties
- `ForeignKeyMetadata`: Foreign key relationships
- `ViewMetadata`: View definitions and dependencies
- `MaterializedViewMetadata`: Materialized view definitions
- `FunctionMetadata`: Stored function definitions
- `ProcedureMetadata`: Stored procedure definitions
- `ExternalTableMetadata`: External/foreign table definitions

**Supporting Types**:
- `DependentColumn`: Represents column dependencies in views
- `IdentityGeneration`: Identity column generation strategies

#### 3. Database Drivers

##### MySQL/TiDB Driver (`src/db/mysql/`)
- Implements the `DB` trait for MySQL and TiDB
- Uses sqlx for database connectivity
- Handles MySQL-specific metadata queries
- **Files**:
  - `mod.rs`: Module exports
  - `sync.rs`: Driver implementation and metadata sync logic

##### PostgreSQL Driver (`src/db/postgres/`)
- Implements the `DB` trait for PostgreSQL
- Handles PostgreSQL-specific features (schemas, extensions, etc.)
- **Files**:
  - `mod.rs`: Module exports
  - `sync.rs`: Driver implementation and metadata sync logic
  - `system.rs`: PostgreSQL system-level operations

#### 4. Utility Modules

- `src/db/util.rs`: Common utility functions for database operations
- `src/db/error.rs`: Error types and handling
- `src/tests/`: Test utilities and helpers

## Module Structure

```
dbmeta/
├── src/
│   ├── lib.rs                  # Library entry point
│   ├── db/                     # Core database module
│   │   ├── mod.rs              # DB trait and engine abstraction
│   │   ├── store.rs            # Metadata structures
│   │   ├── error.rs            # Error definitions
│   │   ├── util.rs             # Utility functions
│   │   ├── mysql/              # MySQL/TiDB implementation
│   │   │   ├── mod.rs
│   │   │   └── sync.rs
│   │   └── postgres/           # PostgreSQL implementation
│   │       ├── mod.rs
│   │       ├── sync.rs
│   │       └── system.rs
│   └── tests/                  # Test utilities
│       ├── mod.rs
│       └── utils.rs
├── Cargo.toml                  # Project dependencies and features
└── README.md                   # Project documentation
```

## Feature Flags

The library uses Cargo features to enable/disable database drivers:

- `db-all`: Enables all database drivers (MySQL, TiDB, PostgreSQL)
- `db-mysql`: Enables MySQL support
- `db-tidb`: Enables TiDB support
- `db-postgres`: Enables PostgreSQL support

## Dependencies

### Core Dependencies
- **tokio**: Async runtime (v1.20.0)
- **async-trait**: Async trait support (v0.1.68)
- **sqlx**: SQL database driver (v0.7) - optional, enabled per database feature

### Utility Dependencies
- **url**: URL parsing (v2)
- **regex**: Regular expressions (v1.10.4)
- **version-compare**: Version comparison (v0.2.0)
- **lazy_static**: Lazy static initialization (v1.5)
- **phf**: Compile-time hash maps (v0.12.1)
- **dotenvy**: Environment variable loading (v0.15)

## Usage Patterns

### Basic Usage

1. **Configuration**: Create a `ConnectionConfig` with database credentials
2. **Driver Creation**: Use `create_driver()` to instantiate the appropriate driver
3. **Metadata Sync**: Call `sync_instance()` or `sync_database()` to retrieve metadata

### Example Workflow

```rust
// 1. Configure connection
let cfg = ConnectionConfig {
    engine: Engine::MYSQL,
    host: "localhost".into(),
    port: 3306,
    username: "user".into(),
    password: "pass".into(),
    database: "mydb".into(),
};

// 2. Create driver
let driver = create_driver(&cfg).await?;

// 3. Retrieve metadata
let instance = driver.sync_instance().await?;
let database = driver.sync_database().await?;
```

## Design Patterns

### 1. Trait-based Polymorphism
The `DB` trait provides a common interface for all database backends, allowing runtime polymorphism through dynamic dispatch (`Box<dyn DB>`).

### 2. Factory Pattern
The `create_driver()` function acts as a factory, creating the appropriate driver based on the `Engine` enum.

### 3. Feature-gated Compilation
Database drivers are conditionally compiled based on feature flags, reducing binary size when not all databases are needed.

### 4. Async/Await
All I/O operations are asynchronous, leveraging Tokio for efficient concurrent operations.

## Extending the Library

### Adding a New Database Driver

1. Create a new module under `src/db/` (e.g., `src/db/oracle/`)
2. Implement the `DB` trait for your driver
3. Add a new `Engine` variant (feature-gated)
4. Update `create_driver()` to handle the new engine
5. Add appropriate feature flags in `Cargo.toml`
6. Add necessary dependencies (e.g., database client library)

### Adding New Metadata Types

1. Define the structure in `src/db/store.rs`
2. Update relevant metadata containers (e.g., `SchemaMetadata`)
3. Implement extraction logic in database-specific drivers

## Testing

Test utilities are provided in `src/tests/`:
- `init_mysql_test_service()`: Initialize MySQL test environment
- `init_pg_test_service()`: Initialize PostgreSQL test environment

## License

This project is licensed under the Apache License. See the [LICENSE](LICENSE) file for details.

## Acknowledgements

Some code is translated from the open-source project [Bytebase](https://github.com/bytebase/bytebase).

## Contributing

When contributing to this project:
1. Follow Rust naming conventions and style guidelines
2. Add tests for new functionality
3. Update documentation for API changes
4. Ensure feature flags work correctly
5. Test with all supported database backends
6. Keep the unified interface consistent across drivers
