select max(p.id) filter (where p.user_id > 1) over () as top, max(p.id) filter (where p.user_id > 1) as plain_top from posts p
