select count(*) as n, (select json_object('k', any_value(u.name), 'c', count(*)) from posts p group by p.user_id order by p.user_id limit 1) as o from users u where false
