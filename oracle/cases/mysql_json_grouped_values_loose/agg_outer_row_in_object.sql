select json_object('n', count(*), 's', (select json_object('k', u.id, 'c', count(*)) from posts p group by p.user_id order by p.user_id limit 1)) as o from users u where false
