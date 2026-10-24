select distinct p.user_id, max(p.id) as top from posts p group by p.user_id
