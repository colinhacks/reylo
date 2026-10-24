select sum(p.id) filter (where p.user_id > 1) over () as total from posts p
