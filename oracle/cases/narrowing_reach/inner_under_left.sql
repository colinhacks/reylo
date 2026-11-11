select o.id, c.author_id from orders o left join (comments c join users u on u.id = c.author_id) on c.id = o.id
