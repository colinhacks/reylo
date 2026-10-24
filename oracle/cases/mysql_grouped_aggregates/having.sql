select p.user_id, max(p.title) as top from posts p group by p.user_id having count(*) > 1
