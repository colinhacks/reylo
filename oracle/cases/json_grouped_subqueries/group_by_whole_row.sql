select (select jsonb_build_object('a', jsonb_agg(p.id), 'm', max(p.n), 'c', count(*)) from posts p group by p order by 1 limit 1) as j
