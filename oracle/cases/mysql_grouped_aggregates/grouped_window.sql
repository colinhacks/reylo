select p.user_id, max(p.id) as top, sum(max(p.id)) over () as total from posts p group by p.user_id
