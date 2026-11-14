select (select jsonb_build_object('t', p.title, 'c', count(*), 'a', jsonb_agg(p.id)) from posts p group by p.title having count(*) > 0 and max(p.score) is null order by p.title limit 1) as j
