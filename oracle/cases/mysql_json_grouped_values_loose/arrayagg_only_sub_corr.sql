select u.id, (select json_object('name', p.title, 'x', concat(p.title, ''), 'a', json_arrayagg(p.id)) from posts p where p.user_id = u.id) as o from users u
