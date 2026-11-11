select (select json_object('m', (select max(p.id)), 't', p.title, 'x', concat(p.title, '')) from posts p where false) as o
