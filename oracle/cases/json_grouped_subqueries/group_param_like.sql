select (select jsonb_build_object('c', count(*), 'm', max(p.n), 'a', jsonb_agg(p.score)) from posts p group by p.user_id, p.tag order by 1 limit 1) as j
