select (select json_object('k', any_value(p.title), 'o', json_object('a', json_arrayagg(p.id))) from posts p where false) as o
