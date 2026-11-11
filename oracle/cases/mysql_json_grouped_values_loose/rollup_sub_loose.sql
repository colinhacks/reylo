select (select json_object('u', p.user_id, 't', p.title) from posts p group by p.user_id with rollup order by p.user_id limit 1) as o
