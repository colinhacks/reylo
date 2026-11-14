select (select json_object('t', p.id = 1, 'b', p.title like 'a%', 'n', count(*)) from posts p where p.user_id = u.id) as o from users u
