select u.id, (select json_object('a', json_arrayagg(json_object('k', u.name, 'p', p.id))) from posts p group by p.user_id order by p.user_id limit 1) as o from users u group by u.id with rollup
