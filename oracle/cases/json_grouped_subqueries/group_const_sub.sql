select (select jsonb_build_object('c', count(*), 'm', max(p.n), 'a', jsonb_agg(p.score)) from posts p group by 1 + 0) as j
