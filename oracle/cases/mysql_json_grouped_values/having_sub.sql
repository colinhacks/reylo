select (select json_object('t', p.title, 'c', count(*), 'a', json_arrayagg(p.id)) from posts p group by p.title having max(p.score) is null order by p.title limit 1) as o
