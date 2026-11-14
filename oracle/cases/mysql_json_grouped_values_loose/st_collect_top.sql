select json_object('k', u.name, 'g', st_collect(point(u.id, u.id))) as o from users u where false
