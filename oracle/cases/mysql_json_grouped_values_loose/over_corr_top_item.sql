select count(*) as n, (select json_object('c', count(*), 'w', row_number() over (order by u.id)) from posts p) as o from users u where false
