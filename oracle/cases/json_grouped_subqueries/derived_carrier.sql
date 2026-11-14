select d.j from (select (select jsonb_build_object('a', jsonb_agg(p.id), 'm', max(p.n), 'k', p.user_id || '') from posts p group by p.user_id order by 1 limit 1) as j) d
