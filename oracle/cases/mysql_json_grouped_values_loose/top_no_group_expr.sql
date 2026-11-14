select json_object('x', concat(u.name, ''), 'i', u.id + 0, 't', u.id = 1, 'n', count(*)) as o from users u where false
