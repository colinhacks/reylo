select u.id, (select max(p.id) from posts p where p.user_id = u.id group by p.user_id) as top from users u
