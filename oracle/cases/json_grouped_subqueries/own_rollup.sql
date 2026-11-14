select (select jsonb_build_object('t', p.title, 'a', jsonb_agg(p.id)) from posts p group by rollup (p.title) order by 1 limit 1) as j
