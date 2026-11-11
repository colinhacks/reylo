select o.* from orders o join users u on u.id = o.user_id where nickname = :n
