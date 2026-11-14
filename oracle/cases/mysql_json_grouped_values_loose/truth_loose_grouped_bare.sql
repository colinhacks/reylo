select (select json_object('t', p.n = 1, 'b', p.title like 'a%', 'n', count(*)) from posts p group by p.user_id order by p.user_id limit 1) as o
