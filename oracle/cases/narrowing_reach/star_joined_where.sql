select o.*, u.* from orders o left join users u on u.id = o.user_id where u.nickname like :p and o.note is not null
