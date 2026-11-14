select (select json_object('k', any_value(p.title), 'g', st_collect(point(p.id, p.id))) from posts p where false) as o
