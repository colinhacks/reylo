select count(*) as n, (select json_object('c', count(*), 'g', group_concat(p.title separator 'x')) from posts p where p.id > 0) as o, max(u.id) as m from users u where false
