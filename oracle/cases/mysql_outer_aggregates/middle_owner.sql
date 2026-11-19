select u.id, (select (select count(p.title) from comments c limit 1) from posts p where p.user_id = u.id limit 1) as n from users u
