select (select jsonb_build_object('o', jsonb_build_object('a', jsonb_agg(p.id), 'm', max(p.n), 't', p.tag)) from posts p group by p.tag order by 1 limit 1) as j
