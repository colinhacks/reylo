select (select jsonb_build_object('k', lower(p.title), 'c', count(*), 'a', jsonb_agg(p.score)) from posts p group by lower(p.title) order by 1 limit 1) as j
