select json_object('name', u.name, 'x', concat(u.name, '')) as o from users u having count(*) >= 0
