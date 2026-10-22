select jsonb_build_object('name', u.name, 'n', count(*)) as j from users u join posts p on p.user_id = u.id group by u.id
