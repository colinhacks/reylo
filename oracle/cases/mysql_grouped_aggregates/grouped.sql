select p.user_id, sum(p.id) as total, avg(p.id) as mean, min(p.title) as first_title, max(p.published_at) as last_at, sum(p.score) as score from posts p group by p.user_id
