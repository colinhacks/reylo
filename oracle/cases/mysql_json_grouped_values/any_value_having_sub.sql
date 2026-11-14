select u.id, (select json_object('k', any_value(p.title)) from posts p where p.user_id = u.id having count(*) >= 0) as o from users u
