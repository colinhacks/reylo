select (select json_object('n', count(*), 's', coalesce((select json_object('c', count(*)) from posts p where p.user_id = x.id), json_object())) from users x where false) as o
