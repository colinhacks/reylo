select * from (select id, id, (select count(*) from posts p where p.user_id = users.id) from users) d (a, b, c)
