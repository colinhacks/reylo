select json_object('k', any_value(u.name), 'x', concat(any_value(u.name), ''), 'g', st_collect(point(u.id, u.id))) as o from users u where false
