# Test Fixtures

This directory contains predefined DDL (Data Definition Language) scripts for testing the dbmeta library.

## Files

### MySQL Fixtures

- **`mysql_schema.sql`**: Comprehensive MySQL schema with:
  - 4 tables (customers, products, orders, order_items)
  - Various column types (INT, VARCHAR, TEXT, DECIMAL, TIMESTAMP, BOOLEAN)
  - Primary keys with AUTO_INCREMENT
  - Foreign key relationships with CASCADE and RESTRICT rules
  - Multiple indexes (primary, unique, composite)
  - One view (customer_orders)
  - Test data for validation

- **`mysql_routines.sql`**: MySQL stored procedures and functions:
  - `get_customer_orders` procedure - retrieves orders for a customer by email
  - `calculate_order_total` function - calculates the total amount for an order
  - These are loaded and validated in the test suite

### PostgreSQL Fixtures

- **`postgres_schema.sql`**: Comprehensive PostgreSQL schema with:
  - 2 custom schemas (sales, inventory)
  - Custom enum type (order_status)
  - 4 tables across different schemas
  - Foreign key relationships across schemas
  - Multiple indexes with various types (btree)
  - One regular view (customer_order_summary)
  - One materialized view (monthly_sales)
  - One function (calculate_order_total)
  - Test data for validation

## Purpose

These fixtures serve multiple purposes:

1. **Metadata Validation**: Provide a known schema structure to validate that dbmeta correctly extracts:
   - Table definitions
   - Column types, constraints, and defaults
   - Index properties (primary, unique, composite)
   - Foreign key relationships and cascade rules
   - View and materialized view definitions
   - Schema organization (PostgreSQL)

2. **Test Coverage**: Exercise all metadata features that dbmeta supports:
   - Different column types and constraints
   - Various index configurations
   - Complex foreign key relationships
   - Multi-schema structures (PostgreSQL)
   - Custom types (PostgreSQL)

3. **Regression Testing**: Ensure that changes to dbmeta don't break metadata extraction

## Usage

These fixtures are automatically loaded by the test utilities:

```rust
// MySQL
init_mysql_test_schema().await?;

// PostgreSQL
init_postgres_test_schema().await?;
```

The initialization functions use the respective database CLI tools (mysql/psql) to execute the scripts.

## Schema Design

Both schemas follow a similar e-commerce domain model:
- **Customers**: User information
- **Products**: Product catalog
- **Orders**: Customer orders
- **Order Items**: Line items in orders

This provides a realistic, relatable schema that exercises common database patterns like:
- One-to-many relationships
- Many-to-many relationships (through junction tables)
- Cascading deletes
- Referential integrity constraints

## Maintenance

When adding new features to dbmeta that extract additional metadata:

1. Update the relevant fixture file to include examples of the new feature
2. Update the corresponding validation test to assert the new metadata is correctly extracted
3. Document the new fixture elements in this README

## Requirements

To run tests that use these fixtures, you need:

- MySQL 8.0+ (for MySQL tests)
- PostgreSQL 15+ (for PostgreSQL tests)
- `mysql` command-line client (for MySQL tests)
- `psql` command-line client (for PostgreSQL tests)

These are typically installed with the database server or available in database client packages.
