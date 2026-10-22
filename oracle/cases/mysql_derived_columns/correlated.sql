select d.id, d.n from (select u.id, (select count(*) from posts p where p.user_id = u.id) as n from users u) d
