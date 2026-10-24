select u.name, (select max(p.id) over () from posts p where p.id = u.id) as own from users u
