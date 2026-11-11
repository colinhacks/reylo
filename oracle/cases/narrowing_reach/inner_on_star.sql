select c.* from comments c join users u on u.id = c.parent_id
