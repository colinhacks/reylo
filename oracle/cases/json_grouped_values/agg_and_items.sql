select jsonb_build_object('name', u.name, 'n', count(p.id), 'titles', jsonb_agg(p.title)) as j from users u join posts p on p.user_id = u.id group by u.id
