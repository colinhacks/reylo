select u.id, (select jsonb_build_object('a', jsonb_agg(jsonb_build_object('k', u.id, 'p', p.id))) from posts p group by p.user_id order by 1 limit 1) as j from users u group by rollup (u.id)
