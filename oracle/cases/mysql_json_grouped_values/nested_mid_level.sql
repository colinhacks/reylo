select (select json_object('t', p.title, 'm', (select max(p.id)), 'a', (select json_arrayagg(p.id))) from posts p group by p.title order by p.title limit 1) as o
