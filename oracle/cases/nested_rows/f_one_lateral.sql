select p.id, u from posts p left join lateral (select id, name from users where id = p.author_id) u on true
