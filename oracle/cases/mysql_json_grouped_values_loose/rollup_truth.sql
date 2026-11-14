select u.id, (select json_object('t', u.id = u.id, 'c', count(*)) from posts p group by p.user_id order by p.user_id limit 1) as o from users u group by u.id with rollup
