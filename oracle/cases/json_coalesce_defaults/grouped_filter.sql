select jsonb_build_object('name', u.name, 'ids', coalesce(jsonb_agg(p.id) filter (where p.id > 0), '{}')) as j from users u join posts p on p.user_id = u.id group by u.name
