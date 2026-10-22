select u.name, s.n from users u join (select p.user_id as n from posts p union select c.post_id from comments c) s on s.n = u.id
