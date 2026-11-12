select (select json_object('t', p.title, 'u', p.user_id, 'c', count(*)) from posts p group by p.user_id with rollup order by p.user_id limit 1) as o
