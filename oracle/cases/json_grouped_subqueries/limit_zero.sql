select (select jsonb_build_object('t', p.title, 'a', jsonb_agg(p.id)) from posts p group by p.title limit 0) as j
