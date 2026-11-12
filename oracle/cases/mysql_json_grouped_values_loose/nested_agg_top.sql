select json_object('m', (select max(u.id)), 't', u.name) as o from users u where false
