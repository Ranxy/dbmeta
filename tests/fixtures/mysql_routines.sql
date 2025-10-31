-- MySQL Stored Procedures and Functions
-- Note: These need to be executed with DELIMITER handling

DROP PROCEDURE IF EXISTS get_customer_orders;
DROP FUNCTION IF EXISTS calculate_order_total;

-- Stored Procedure
CREATE PROCEDURE get_customer_orders(IN customer_email VARCHAR(255))
BEGIN
    SELECT 
        o.order_id,
        o.order_date,
        o.total_amount,
        o.status
    FROM orders o
    JOIN customers c ON o.customer_id = c.customer_id
    WHERE c.email = customer_email
    ORDER BY o.order_date DESC;
END;

-- Stored Function
CREATE FUNCTION calculate_order_total(order_id_param INT)
RETURNS DECIMAL(10,2)
DETERMINISTIC
READS SQL DATA
BEGIN
    DECLARE total DECIMAL(10,2);
    SELECT SUM(quantity * unit_price * (1 - discount/100))
    INTO total
    FROM order_items
    WHERE order_id = order_id_param;
    RETURN IFNULL(total, 0.00);
END;
