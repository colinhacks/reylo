select u.name, s.title from users u cross join lateral (select id, title from posts where author_id = u.id) s where u.id = :u and s.id = :p
