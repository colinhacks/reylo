select count(*) as n, (select json_object('k', any_value(u.name), 'c', count(*)) from posts p) as o from users u where false
