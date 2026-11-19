SELECT u.id, u.name, o.total, o.note, o.id AS order_id FROM users u LEFT JOIN orders o ON o.user_id = u.id ORDER BY u.id, o.id
