select u.id, l.j from users u left join lateral (select jsonb_build_object('a', jsonb_agg(p.id), 'c', count(*)) as j from posts p where p.user_id = u.id) l on true group by u.id, l.j
