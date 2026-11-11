select u.id as user_id, p.id as post_id from users u cross join lateral (select id from posts where author_id = u.id order by id limit 2) p
