select sum(p.id) over (partition by p.user_id) as total from posts p
