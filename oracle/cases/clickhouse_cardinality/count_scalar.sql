SELECT (SELECT count() FROM orders) AS orders, name FROM users ORDER BY id, name
