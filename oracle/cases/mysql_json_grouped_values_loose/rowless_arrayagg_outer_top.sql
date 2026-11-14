select json_object('n', count(*), 's', (select json_object('a', json_arrayagg(json_object('k', u.name, 'p', p.id))) from posts p)) as o from users u where false
