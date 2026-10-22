select jsonb_build_object('name', u.name, 'best', max(p.title)) as j from users u join posts p on p.user_id = u.id group by u.id, u.name
