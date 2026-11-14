select p.id, (select u from (select id, name from users where id = p.author_id) u) as author from posts p
