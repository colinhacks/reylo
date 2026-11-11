select json_object('n', count(*), 's', (select json_object('k', any_value(u.name)) from posts p order by p.id limit 1)) as o from users u where false
