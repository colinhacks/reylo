select jsonb_build_object('n', count(*), 'best', max(p.title)) as j, lower(p.title) as t from posts p group by t
