with a as (select p.id from posts p union select c.post_id from comments c) select u.name, u.email, a.id from users u join posts p on p.user_id = u.id join a on a.id = p.id
