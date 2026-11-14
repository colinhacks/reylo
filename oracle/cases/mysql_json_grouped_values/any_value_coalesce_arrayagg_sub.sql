select (select json_object('k', any_value(p.title), 'a', coalesce(json_arrayagg(p.id), json_array())) from posts p where false) as o
