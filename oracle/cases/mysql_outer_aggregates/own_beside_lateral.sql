select p.id, (select d.c from users u, lateral (select count(u.name) as c from comments) d limit 1) as x from posts p
