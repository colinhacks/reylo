select json_object('name', u.name, 'x', concat(u.name, ''), 'a', json_arrayagg(u.id)) as o from users u where false
