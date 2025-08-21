# dbmeta

dbmeta is an open source Rust library for managing and interacting with multiple database backends, including MySQL and PostgreSQL. It provides a unified interface for obtaining database metadata, making it easier to build Rust applications that require metadata from multiple databases.

## Getting Started

### Installation

Add dbmeta to your `Cargo.toml`:

```toml
[dependencies]
dbmeta = { version =0.1, features=["db-all"]}
```

### Usage Example

```rust
use dbmeta::db;

fn main() {
    let cfg = db::ConnectionConfig {
        engine: db::Engine::MYSQL,
        host: "localhost".into(),
        port: 3306,
        username: "username".into(),
        password: "password".into(),
        database: "database".into(),
    };
    let driver = db::create_driver(&cfg).await.unwrap();

    let instance = driver.sync_instance().await.unwrap();
    println!("Database instance: {:?}", instance);

    let databases = driver.sync_database().await.unwrap();
    println!("Databases: {:?}", databases);
}
```

## License

This project is licensed under the Apache License. See the [LICENSE](LICENSE) file for details.

## Acknowledgements

Some codes are translated from the open source project Bytebase.
