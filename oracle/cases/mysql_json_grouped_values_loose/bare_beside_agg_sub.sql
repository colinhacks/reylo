select (select json_object('name', p.title, 'x', concat(p.title, ''), 't', p.id = 1, 'n', count(*), 'a', json_arrayagg(p.id)) from posts p where p.user_id = u.id) as o from users u
