select (select jsonb_build_object('t', p.title, 'c', count(u.*)) from posts p group by p.title order by p.title limit 1) as j from users u
