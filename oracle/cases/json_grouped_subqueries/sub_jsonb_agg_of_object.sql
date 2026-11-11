select (select jsonb_agg(jsonb_build_object('t', p.title, 'm', p.n)) from posts p group by p.user_id order by 1 limit 1) as j
