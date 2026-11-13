select (select json_object('n', count(*), 's', (select json_object('a', json_arrayagg(json_object('k', x.name, 'p', p.id))) from posts p)) from users x where false) as o
