select (select jsonb_build_object('t', p.title, 's', string_agg(u.name, ',' order by u.id)) from posts p group by p.title order by p.title limit 1) as j from users u
