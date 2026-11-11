select (select json_object('n', count(*), 's', (select json_object('c', count(*), 'w', row_number() over (partition by any_value(x.id))) from posts p)) from users x where false) as o
