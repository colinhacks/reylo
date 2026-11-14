select (select json_object('t', p.title, 'm', max(u.id), 'a', json_arrayagg(u.name)) from posts p group by p.title order by p.title limit 1) as o from users u
