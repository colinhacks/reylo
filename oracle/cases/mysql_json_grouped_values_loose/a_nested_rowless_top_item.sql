select count(*) as n, (select json_object('k', u.name) from posts p order by p.id limit 1) as o from users u where false
