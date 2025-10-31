# Testing Guide

This document describes how to run tests for the dbmeta project both locally and in CI/CD.

## Overview

The dbmeta project has tests for multiple database backends:
- MySQL/TiDB
- PostgreSQL

Tests require actual database instances to be running and accessible.

## Running Tests Locally

### Prerequisites

1. **Install Rust**: Make sure you have Rust and Cargo installed
2. **Install Docker** (recommended): For easy database setup
3. **Or install databases directly**: MySQL 8.0+ and/or PostgreSQL 15+

### Quick Start with Docker

#### 1. Start MySQL
```bash
docker run -d \
  --name dbmeta-mysql-test \
  -e MYSQL_ROOT_PASSWORD=test_password \
  -e MYSQL_DATABASE=test_db \
  -p 3306:3306 \
  mysql:8.0
```

#### 2. Start PostgreSQL
```bash
docker run -d \
  --name dbmeta-postgres-test \
  -e POSTGRES_USER=test_user \
  -e POSTGRES_PASSWORD=test_password \
  -e POSTGRES_DB=test_db \
  -p 5432:5432 \
  postgres:15
```

#### 3. Configure Environment Variables

Copy the example environment file and update if needed:
```bash
cp .env.example .env
```

Edit `.env` with your database credentials:
```env
# MySQL configuration
TEST_MYSQL_DB_HOST=localhost
TEST_MYSQL_DB_PORT=3306
TEST_MYSQL_DB_USERNAME=root
TEST_MYSQL_DB_PASSWORD=test_password
TEST_MYSQL_DB_DATABASE=test_db

# PostgreSQL configuration
TEST_POSTGRES_DB_HOST=localhost
TEST_POSTGRES_DB_PORT=5432
TEST_POSTGRES_DB_USERNAME=test_user
TEST_POSTGRES_DB_PASSWORD=test_password
TEST_POSTGRES_DB_DATABASE=test_db
```

#### 4. Run Tests

Run all tests:
```bash
cargo test --features db-all
```

Run MySQL tests only:
```bash
cargo test --features db-mysql
```

Run PostgreSQL tests only:
```bash
cargo test --features db-postgres
```

Run specific test:
```bash
cargo test --features db-mysql test_get_version
```

### Cleanup

Stop and remove test containers:
```bash
docker stop dbmeta-mysql-test dbmeta-postgres-test
docker rm dbmeta-mysql-test dbmeta-postgres-test
```

## Environment Variables

The test suite reads configuration from environment variables with the following pattern:

### MySQL/TiDB
- `TEST_MYSQL_DB_HOST` - Database host (default: `localhost`)
- `TEST_MYSQL_DB_PORT` - Database port (default: `3306`)
- `TEST_MYSQL_DB_USERNAME` - Database username (default: empty)
- `TEST_MYSQL_DB_PASSWORD` - Database password (default: empty)
- `TEST_MYSQL_DB_DATABASE` - Database name (default: empty)

### PostgreSQL
- `TEST_POSTGRES_DB_HOST` - Database host (default: `localhost`)
- `TEST_POSTGRES_DB_PORT` - Database port (default: `5432`)
- `TEST_POSTGRES_DB_USERNAME` - Database username (default: empty)
- `TEST_POSTGRES_DB_PASSWORD` - Database password (default: empty)
- `TEST_POSTGRES_DB_DATABASE` - Database name (default: empty)

## CI/CD Testing

The project uses GitHub Actions for automated testing. The workflow:

1. **Triggers**: On push to `main`/`master` branches and on pull requests
2. **Services**: Automatically starts MySQL and PostgreSQL containers
3. **Test Execution**: Runs tests for each database backend separately
4. **Additional Checks**: Runs formatting checks and clippy lints

### Workflow Files

- `.github/workflows/ci.yml` - Main CI/CD workflow

The CI workflow includes:
- **Test Suite**: Runs all tests with database services
- **Lint**: Checks code formatting and runs clippy
- **Build**: Verifies the project builds with different feature flags

## Test Structure

### Test Organization

Tests are organized by database driver:
- `src/db/mysql/sync.rs` - MySQL/TiDB tests
- `src/db/postgres/sync.rs` - PostgreSQL tests

### Test Utilities

Test utilities are in `src/tests/utils.rs` and provide helper functions:
- `init_mysql_test_service()` - Creates MySQL connection config from env vars
- `init_pg_test_service()` - Creates PostgreSQL connection config from env vars

### What Tests Validate

The tests query database metadata including:
- Database version information
- List of databases
- Schemas (PostgreSQL)
- Tables and their structures
- Columns and data types
- Indexes
- Views and materialized views
- Foreign keys
- Functions and procedures

## Troubleshooting

### Tests Fail with "pool timed out"

This usually means the database is not running or not accessible:
1. Check that database containers are running: `docker ps`
2. Verify environment variables are set correctly
3. Check database logs: `docker logs dbmeta-mysql-test` or `docker logs dbmeta-postgres-test`
4. Wait a few seconds after starting databases before running tests

### Connection Refused

1. Ensure ports 3306 (MySQL) and 5432 (PostgreSQL) are not already in use
2. Check firewall settings
3. Verify the database containers started successfully

### Wrong Credentials

1. Double-check your `.env` file matches the database configuration
2. If using Docker, ensure the environment variables match what you passed to `docker run`

## Contributing

When adding new tests:
1. Follow existing test patterns
2. Use the test utility functions for database connections
3. Ensure tests clean up after themselves
4. Add documentation for any new test requirements
5. Verify tests pass both locally and in CI

## Additional Resources

- [Rust Testing Documentation](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Docker Documentation](https://docs.docker.com/)
