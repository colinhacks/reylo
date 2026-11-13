select (select json_object('n', count(*), 's', (select json_object('m', max(concat(x.name, p.id)), 'c', count(*)) from posts p)) from users x where false) as o
