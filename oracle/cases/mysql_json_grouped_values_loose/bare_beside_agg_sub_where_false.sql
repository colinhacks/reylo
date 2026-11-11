select (select json_object('name', p.title, 'x', concat(p.title, ''), 'a', json_arrayagg(p.id)) from posts p where false) as o
