-- PostgreSQL Test Schema
-- This schema is designed to exercise all metadata features that dbmeta extracts

-- Create a custom schema
CREATE SCHEMA IF NOT EXISTS sales;
CREATE SCHEMA IF NOT EXISTS inventory;

-- Set search path to include our schemas
SET search_path TO sales, inventory, public;

-- Create a custom type (enum)
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'order_status') THEN
        CREATE TYPE sales.order_status AS ENUM ('pending', 'processing', 'shipped', 'delivered', 'cancelled');
    END IF;
END$$;

-- Drop existing objects if they exist (in reverse dependency order)
DROP VIEW IF EXISTS sales.customer_order_summary CASCADE;
DROP MATERIALIZED VIEW IF EXISTS sales.monthly_sales CASCADE;
DROP TABLE IF EXISTS sales.order_items CASCADE;
DROP TABLE IF EXISTS sales.orders CASCADE;
DROP TABLE IF EXISTS sales.customers CASCADE;
DROP TABLE IF EXISTS inventory.products CASCADE;
DROP FUNCTION IF EXISTS sales.calculate_order_total(INT) CASCADE;

-- Create customers table in sales schema
CREATE TABLE sales.customers (
    customer_id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    phone VARCHAR(20),
    address TEXT,
    city VARCHAR(100),
    country VARCHAR(100) DEFAULT 'USA',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE
);

COMMENT ON TABLE sales.customers IS 'Customer information';
COMMENT ON COLUMN sales.customers.customer_id IS 'Unique customer identifier';
COMMENT ON COLUMN sales.customers.email IS 'Customer email address';

-- Create indexes on customers
CREATE INDEX idx_customers_email ON sales.customers(email);
CREATE INDEX idx_customers_name ON sales.customers(last_name, first_name);
CREATE INDEX idx_customers_city ON sales.customers(city);

-- Create products table in inventory schema
CREATE TABLE inventory.products (
    product_id SERIAL PRIMARY KEY,
    product_name VARCHAR(200) NOT NULL,
    description TEXT,
    price NUMERIC(10, 2) NOT NULL,
    stock_quantity INTEGER DEFAULT 0,
    category VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

COMMENT ON TABLE inventory.products IS 'Product catalog';
COMMENT ON COLUMN inventory.products.product_id IS 'Unique product identifier';

-- Create indexes on products
CREATE INDEX idx_products_category ON inventory.products(category);
CREATE INDEX idx_products_price ON inventory.products(price);

-- Create orders table with foreign key and custom enum type
CREATE TABLE sales.orders (
    order_id SERIAL PRIMARY KEY,
    customer_id INTEGER NOT NULL,
    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    total_amount NUMERIC(10, 2) NOT NULL DEFAULT 0.00,
    status sales.order_status DEFAULT 'pending',
    shipping_address TEXT,
    notes TEXT,
    CONSTRAINT fk_orders_customer 
        FOREIGN KEY (customer_id) 
        REFERENCES sales.customers(customer_id) 
        ON DELETE CASCADE 
        ON UPDATE CASCADE
);

COMMENT ON TABLE sales.orders IS 'Customer orders';
COMMENT ON COLUMN sales.orders.order_id IS 'Unique order identifier';

-- Create indexes on orders
CREATE INDEX idx_orders_customer ON sales.orders(customer_id);
CREATE INDEX idx_orders_date ON sales.orders(order_date);
CREATE INDEX idx_orders_status ON sales.orders(status);

-- Create order_items table with composite foreign keys
CREATE TABLE sales.order_items (
    order_item_id SERIAL PRIMARY KEY,
    order_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price NUMERIC(10, 2) NOT NULL,
    discount NUMERIC(5, 2) DEFAULT 0.00,
    CONSTRAINT fk_order_items_order 
        FOREIGN KEY (order_id) 
        REFERENCES sales.orders(order_id) 
        ON DELETE CASCADE 
        ON UPDATE CASCADE,
    CONSTRAINT fk_order_items_product 
        FOREIGN KEY (product_id) 
        REFERENCES inventory.products(product_id) 
        ON DELETE RESTRICT 
        ON UPDATE CASCADE
);

COMMENT ON TABLE sales.order_items IS 'Items in orders';

-- Create indexes on order_items
CREATE INDEX idx_order_items_order ON sales.order_items(order_id);
CREATE INDEX idx_order_items_product ON sales.order_items(product_id);

-- Create a regular view
CREATE VIEW sales.customer_order_summary AS
SELECT 
    c.customer_id,
    c.email,
    c.first_name,
    c.last_name,
    COUNT(o.order_id) as total_orders,
    SUM(o.total_amount) as total_spent
FROM sales.customers c
LEFT JOIN sales.orders o ON c.customer_id = o.customer_id
GROUP BY c.customer_id, c.email, c.first_name, c.last_name;

COMMENT ON VIEW sales.customer_order_summary IS 'Summary of customer orders';

-- Create a materialized view
CREATE MATERIALIZED VIEW sales.monthly_sales AS
SELECT 
    DATE_TRUNC('month', order_date) as month,
    COUNT(*) as order_count,
    SUM(total_amount) as total_revenue
FROM sales.orders
GROUP BY DATE_TRUNC('month', order_date)
ORDER BY month;

CREATE INDEX idx_monthly_sales_month ON sales.monthly_sales(month);

COMMENT ON MATERIALIZED VIEW sales.monthly_sales IS 'Monthly sales statistics';

-- Create a function
CREATE OR REPLACE FUNCTION sales.calculate_order_total(order_id_param INTEGER)
RETURNS NUMERIC(10,2)
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
    total NUMERIC(10,2);
BEGIN
    SELECT SUM(quantity * unit_price * (1 - discount/100))
    INTO total
    FROM sales.order_items
    WHERE order_id = order_id_param;
    
    RETURN COALESCE(total, 0.00);
END;
$$;

COMMENT ON FUNCTION sales.calculate_order_total IS 'Calculate the total amount for an order';

-- Insert test data
INSERT INTO sales.customers (email, first_name, last_name, phone, city, country) VALUES
    ('john.doe@example.com', 'John', 'Doe', '555-0001', 'New York', 'USA'),
    ('jane.smith@example.com', 'Jane', 'Smith', '555-0002', 'Los Angeles', 'USA'),
    ('bob.johnson@example.com', 'Bob', 'Johnson', '555-0003', 'Chicago', 'USA');

INSERT INTO inventory.products (product_name, description, price, stock_quantity, category) VALUES
    ('Laptop', 'High-performance laptop', 999.99, 50, 'Electronics'),
    ('Mouse', 'Wireless mouse', 29.99, 200, 'Electronics'),
    ('Keyboard', 'Mechanical keyboard', 89.99, 100, 'Electronics'),
    ('Monitor', '27-inch 4K monitor', 399.99, 30, 'Electronics'),
    ('Desk Chair', 'Ergonomic office chair', 299.99, 25, 'Furniture');

INSERT INTO sales.orders (customer_id, total_amount, status) VALUES
    (1, 1029.98, 'delivered'),
    (1, 299.99, 'pending'),
    (2, 489.98, 'shipped'),
    (3, 999.99, 'processing');

INSERT INTO sales.order_items (order_id, product_id, quantity, unit_price, discount) VALUES
    (1, 1, 1, 999.99, 0.00),
    (1, 2, 1, 29.99, 0.00),
    (2, 5, 1, 299.99, 0.00),
    (3, 3, 1, 89.99, 0.00),
    (3, 4, 1, 399.99, 0.00),
    (4, 1, 1, 999.99, 0.00);

-- Refresh the materialized view with data
REFRESH MATERIALIZED VIEW sales.monthly_sales;
