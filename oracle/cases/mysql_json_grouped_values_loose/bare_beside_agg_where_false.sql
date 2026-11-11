select json_object('name', u.name, 'x', concat(u.name, ''), 't', u.id = 1, 'm', max(u.id), 'n', count(*)) as o from users u where false
