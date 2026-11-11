select (select json_object('m', (select max(p.id)), 't', any_value(p.title)) from posts p where false) as o
