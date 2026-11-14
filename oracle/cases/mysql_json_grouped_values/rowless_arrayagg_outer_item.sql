select count(*) as n, (select json_object('a', json_arrayagg(concat(any_value(u.name), p.id))) from posts p) as o from users u where false
