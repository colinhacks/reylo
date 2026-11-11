select u.id, json_arrayagg((select json_object('k', u.id) from posts p order by p.id limit 1)) as o from users u group by u.id with rollup
