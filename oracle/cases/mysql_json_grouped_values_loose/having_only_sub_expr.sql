select u.id, (select json_object('x', concat(p.title, ''), 'n', p.n + 0) from posts p where p.user_id = u.id having count(*) >= 0) as o from users u
