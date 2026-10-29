update orders o set total = 1 from users u where o.user_id = u.id and o.note is not null and u.nickname <> '' returning *
