select (select json_object('n', count(*), 's', (select json_object('c', count(*), 'k', any_value(x.name)) from posts p)) from users x where false) as o
