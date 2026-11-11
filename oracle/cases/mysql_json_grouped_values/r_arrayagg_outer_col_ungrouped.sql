select u.id, (select json_object('a', json_arrayagg(json_object('k', u.id, 'p', p.id))) from posts p) as o from users u group by u.id with rollup
