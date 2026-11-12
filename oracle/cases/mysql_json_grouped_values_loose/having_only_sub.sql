select (select json_object('name', p.title, 'x', concat(p.title, '')) from posts p where p.user_id = u.id having count(*) >= 0) as o from users u
