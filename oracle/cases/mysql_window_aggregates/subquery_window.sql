select u.name, (select sum(p.id) over () from posts p where p.id = u.id) as own from users u
