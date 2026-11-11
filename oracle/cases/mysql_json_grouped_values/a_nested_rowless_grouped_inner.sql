select (select json_object('n', count(*), 's', (select json_object('k', any_value(x.name), 'c', count(*)) from posts p group by p.user_id order by p.user_id limit 1)) from users x where false) as o
