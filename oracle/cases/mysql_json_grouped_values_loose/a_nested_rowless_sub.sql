select (select json_object('n', count(*), 's', (select json_object('k', x.name, 'x', concat(x.name, '')) from posts p order by p.id limit 1)) from users x where false) as o
