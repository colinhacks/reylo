select u.id, (select count(*) from posts p where p.user_id = u.id group by p.title with rollup order by p.title limit 1) as c from users u group by u.id
