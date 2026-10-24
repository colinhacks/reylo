select p.user_id, sum(p.id) as total from posts p group by p.user_id
