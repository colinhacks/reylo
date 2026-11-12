select u.id, (select json_object('k', u.id, 'a', json_arrayagg(p.id)) from posts p) as o from users u group by u.id with rollup
