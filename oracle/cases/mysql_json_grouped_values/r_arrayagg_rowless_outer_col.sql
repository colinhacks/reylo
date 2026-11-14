select (select json_object('n', count(*), 'a', (select json_arrayagg(json_object('k', any_value(x.name), 'p', p.id)) from posts p)) from users x where false) as o
