select c.post_id, array_agg(c order by c.id) as comments from comments c group by c.post_id
