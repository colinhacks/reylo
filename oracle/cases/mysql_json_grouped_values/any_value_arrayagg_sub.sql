select (select json_object('k', any_value(p.title), 'a', json_arrayagg(p.id)) from posts p where false) as o
