select count(*) as n, (select json_object('k', u.name, 'x', concat(u.name, ''), 'c', count(*)) from posts p) as o from users u where false
