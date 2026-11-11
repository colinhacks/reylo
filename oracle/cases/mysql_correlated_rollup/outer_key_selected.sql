select u.id, (select u.id from posts p where p.user_id = u.id group by p.title with rollup limit 1) as o from users u group by u.id
