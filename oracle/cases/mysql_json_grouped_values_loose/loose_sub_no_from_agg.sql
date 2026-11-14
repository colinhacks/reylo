select u.id, (select json_object('x', concat(u.name, ''), 'm', max(u.n), 'a', json_arrayagg(u.id))) as o from users u group by u.id
