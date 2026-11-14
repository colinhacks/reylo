select (select json_object('n', count(*), 'w', (select row_number() over (partition by x.id) from posts p order by p.id limit 1)) from users x where false) as o
