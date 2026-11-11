select coalesce((select jsonb_build_object('a', jsonb_agg(p.id), 'm', max(p.title)) from posts p where p.user_id = u.id group by p.tag order by 1 limit 1), '{}') as j from users u
