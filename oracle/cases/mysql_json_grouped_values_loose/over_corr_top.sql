select json_object('n', count(*), 's', (select json_object('c', count(*), 'w', row_number() over (partition by u.id)) from posts p)) as o from users u where false
