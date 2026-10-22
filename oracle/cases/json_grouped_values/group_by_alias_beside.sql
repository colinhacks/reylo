select jsonb_build_object('name', lower(u.name), 'n', count(p.id)) as j, u.id as uid from users u join posts p on p.user_id = u.id group by uid
