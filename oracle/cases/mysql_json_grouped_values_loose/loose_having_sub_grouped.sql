select (select json_object('t', p.title, 'a', json_arrayagg(p.id), 'm', max(p.n)) from posts p group by p.user_id having count(*) > 0 order by p.user_id limit 1) as o
