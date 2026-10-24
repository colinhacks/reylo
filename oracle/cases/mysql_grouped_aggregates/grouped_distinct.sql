select p.user_id, sum(distinct p.id) as total, max(distinct p.title) as top from posts p group by p.user_id
