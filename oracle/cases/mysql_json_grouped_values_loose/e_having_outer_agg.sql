select (select json_object('k', u.name) from posts p having count(u.id) >= 0 order by p.id limit 1) as o from users u where false
