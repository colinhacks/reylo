select (select json_object('k', p.title, 'g', st_collect(point(p.id, p.id))) from posts p where false) as o
