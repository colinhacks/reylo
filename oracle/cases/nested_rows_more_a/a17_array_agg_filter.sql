select c.post_id, array_agg(c) filter (where c.id > 100) as arr from comments c group by c.post_id
