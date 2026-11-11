select u.id, (select json_object('k', u.id, 'c', count(*)) from posts p) as o from users u group by u.id with rollup
