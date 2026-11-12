select (select json_object('t', p.title, 'x', concat(p.title, ''), 's', p.score + 0, 'n', count(*), 'a', json_arrayagg(p.id)) from posts p group by p.user_id order by p.user_id limit 1) as o
