select (select jsonb_build_object('a', json_arrayagg(u.name) is null, 'c', count(*)) from posts p group by p.title order by p.title limit 1) as j from users u
