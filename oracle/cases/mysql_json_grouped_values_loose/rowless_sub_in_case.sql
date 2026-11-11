select (select json_object('n', count(*), 's', case when true then (select json_object('c', count(*)) from posts p where p.user_id = x.id) end) from users x where false) as o
