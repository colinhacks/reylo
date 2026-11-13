select (select jsonb_build_object('c', count(*), 'a', jsonb_agg(p.id), 'm', min(p.n)) from posts p where p.user_id = u.id group by u.name) as j from users u
