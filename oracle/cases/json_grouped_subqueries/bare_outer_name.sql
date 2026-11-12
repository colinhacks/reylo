select (select jsonb_build_object('m', max(name), 'c', count(*)) from posts p group by p.title order by p.title limit 1) as j from users u
