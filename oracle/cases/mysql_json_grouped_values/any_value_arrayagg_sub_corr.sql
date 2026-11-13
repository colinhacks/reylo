select u.id, (select json_object('k', any_value(p.title), 'x', concat(any_value(p.title), ''), 'a', json_arrayagg(p.id)) from posts p where p.user_id = u.id) as o from users u
