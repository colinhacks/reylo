select (select jsonb_build_object('t', p.title, 'm', (select max(p.id)), 'a', (select jsonb_agg(p.id))) from posts p group by p.title order by p.title limit 1) as j
