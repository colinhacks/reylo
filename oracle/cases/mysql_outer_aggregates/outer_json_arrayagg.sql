select b.id, (select json_object('n', count(*), 'a', json_arrayagg((select json_object('k', b.name) from posts p limit 1))) from users x where false) as o from nobody b
