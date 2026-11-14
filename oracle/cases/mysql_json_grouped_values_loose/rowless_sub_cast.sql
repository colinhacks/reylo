select (select json_object('n', count(*), 's', cast((select json_object('c', count(*)) from posts p where p.user_id = x.id) as json)) from users x where false) as o
