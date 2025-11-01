-- MySQL Test Schema
-- This schema is designed to exercise all metadata features that dbmeta extracts

-- Drop existing objects if they exist
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;
DROP TABLE IF EXISTS products;
DROP VIEW IF EXISTS customer_orders;

-- Create customers table with various column types and constraints
CREATE TABLE customers (
    customer_id INT AUTO_INCREMENT PRIMARY KEY COMMENT 'Unique customer identifier',
    email VARCHAR(255) NOT NULL UNIQUE COMMENT 'Customer email address',
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    phone VARCHAR(20),
    address TEXT,
    city VARCHAR(100),
    country VARCHAR(100) DEFAULT 'USA',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT TRUE,
    INDEX idx_email (email),
    INDEX idx_name (last_name, first_name),
    INDEX idx_city (city)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Customer information';

-- Create products table
CREATE TABLE products (
    product_id INT AUTO_INCREMENT PRIMARY KEY COMMENT 'Unique product identifier',
    product_name VARCHAR(200) NOT NULL,
    description TEXT,
    price DECIMAL(10, 2) NOT NULL,
    stock_quantity INT DEFAULT 0,
    category VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_category (category),
    INDEX idx_price (price)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Product catalog';

-- Create orders table with foreign key
CREATE TABLE orders (
    order_id INT AUTO_INCREMENT PRIMARY KEY COMMENT 'Unique order identifier',
    customer_id INT NOT NULL,
    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    total_amount DECIMAL(10, 2) NOT NULL DEFAULT 0.00,
    status VARCHAR(20) DEFAULT 'pending',
    shipping_address TEXT,
    notes TEXT,
    INDEX idx_customer (customer_id),
    INDEX idx_order_date (order_date),
    INDEX idx_status (status),
    CONSTRAINT fk_orders_customer 
        FOREIGN KEY (customer_id) 
        REFERENCES customers(customer_id) 
        ON DELETE CASCADE 
        ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Customer orders';

-- Create order_items table with composite foreign keys
CREATE TABLE order_items (
    order_item_id INT AUTO_INCREMENT PRIMARY KEY COMMENT 'Unique order item identifier',
    order_id INT NOT NULL,
    product_id INT NOT NULL,
    quantity INT NOT NULL DEFAULT 1,
    unit_price DECIMAL(10, 2) NOT NULL,
    discount DECIMAL(5, 2) DEFAULT 0.00,
    INDEX idx_order (order_id),
    INDEX idx_product (product_id),
    CONSTRAINT fk_order_items_order 
        FOREIGN KEY (order_id) 
        REFERENCES orders(order_id) 
        ON DELETE CASCADE 
        ON UPDATE CASCADE,
    CONSTRAINT fk_order_items_product 
        FOREIGN KEY (product_id) 
        REFERENCES products(product_id) 
        ON DELETE RESTRICT 
        ON UPDATE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='Items in orders';

-- Create a view
CREATE VIEW customer_orders AS
SELECT 
    c.customer_id,
    c.email,
    c.first_name,
    c.last_name,
    o.order_id,
    o.order_date,
    o.total_amount,
    o.status
FROM customers c
JOIN orders o ON c.customer_id = o.customer_id;

-- Insert some test data
INSERT INTO customers (email, first_name, last_name, phone, city, country) VALUES
    ('john.doe@example.com', 'John', 'Doe', '555-0001', 'New York', 'USA'),
    ('jane.smith@example.com', 'Jane', 'Smith', '555-0002', 'Los Angeles', 'USA'),
    ('bob.johnson@example.com', 'Bob', 'Johnson', '555-0003', 'Chicago', 'USA');

INSERT INTO products (product_name, description, price, stock_quantity, category) VALUES
    ('Laptop', 'High-performance laptop', 999.99, 50, 'Electronics'),
    ('Mouse', 'Wireless mouse', 29.99, 200, 'Electronics'),
    ('Keyboard', 'Mechanical keyboard', 89.99, 100, 'Electronics'),
    ('Monitor', '27-inch 4K monitor', 399.99, 30, 'Electronics'),
    ('Desk Chair', 'Ergonomic office chair', 299.99, 25, 'Furniture');

INSERT INTO orders (customer_id, total_amount, status) VALUES
    (1, 1029.98, 'completed'),
    (1, 299.99, 'pending'),
    (2, 489.98, 'shipped'),
    (3, 999.99, 'processing');

INSERT INTO order_items (order_id, product_id, quantity, unit_price, discount) VALUES
    (1, 1, 1, 999.99, 0.00),
    (1, 2, 1, 29.99, 0.00),
    (2, 5, 1, 299.99, 0.00),
    (3, 3, 1, 89.99, 0.00),
    (3, 4, 1, 399.99, 0.00),
    (4, 1, 1, 999.99, 0.00);
