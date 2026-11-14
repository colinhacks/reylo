select (select jsonb_build_object('title', p.title, 'name', max(u.name)) from posts p group by p.title order by p.title limit 1) as j from users u
