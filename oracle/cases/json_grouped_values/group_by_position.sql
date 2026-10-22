select jsonb_build_object('name', u.name, 'n', count(p.id)) as j, u.id from users u join posts p on p.user_id = u.id group by 2
