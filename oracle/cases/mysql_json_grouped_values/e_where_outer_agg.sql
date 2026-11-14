select (select json_object('k', any_value(u.name), 't', p.title) from posts p where count(u.id) >= 0 order by p.id limit 1) as o from users u where false
