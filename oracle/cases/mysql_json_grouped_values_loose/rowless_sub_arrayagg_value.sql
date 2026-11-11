select (select json_object('n', count(*), 's', (select json_arrayagg(p.id) from posts p where p.user_id = x.id)) from users x where false) as o
