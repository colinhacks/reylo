select jsonb_build_object('name', u.name, 'titles', coalesce(to_jsonb(array_agg(p.title)), '[]')) as j from users u join posts p on p.user_id = u.id group by u.name
