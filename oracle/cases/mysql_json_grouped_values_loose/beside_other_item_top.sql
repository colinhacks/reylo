select json_object('name', u.name, 'x', concat(u.name, '')) as o, count(*) as n from users u where false
