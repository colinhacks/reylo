select (select jsonb_build_object('t', p.title, 'a', json_arrayagg(p.score)) from posts p group by p.title order by p.title limit 1) as j
