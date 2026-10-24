select p.user_id, sum(p.id) as total, max(p.title) as top from posts p group by p.user_id with rollup
