select json_object('name', u.name, 'f', u.flag, 'x', concat(u.name, ''), 't', u.id = 1, 'n', count(*)) as o from users u
