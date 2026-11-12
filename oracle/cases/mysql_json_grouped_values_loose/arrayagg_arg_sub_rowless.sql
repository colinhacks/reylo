select (select json_object('n', count(*), 'a', json_arrayagg((select json_object('c', count(*), 'k', x.name) from posts p))) from users x where false) as o
