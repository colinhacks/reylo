select (select jsonb_build_object('m', max(p.n + u.id), 'a', jsonb_agg(p.n * u.id), 'c', count(*)) from posts p group by p.tag order by 1 limit 1) as j from users u
