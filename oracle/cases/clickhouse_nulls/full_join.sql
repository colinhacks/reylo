SELECT u.id AS user_id, u.email, o.id AS order_id, o.total FROM users u FULL JOIN orders o ON o.user_id = u.id ORDER BY user_id, order_id
