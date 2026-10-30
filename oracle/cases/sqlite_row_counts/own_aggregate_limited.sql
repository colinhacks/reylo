select (select count(p.id) from posts p where 0 limit 5) as n from users u
