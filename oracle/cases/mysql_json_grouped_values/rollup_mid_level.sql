select (select (select json_object('k', x.id, 'c', count(*)) from posts p group by p.user_id order by p.user_id limit 1) from users x group by x.id with rollup order by x.id limit 1) as o
