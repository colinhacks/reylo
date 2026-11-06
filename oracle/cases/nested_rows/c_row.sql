select p.id, array(select row(c.id, c.body) from comments c where c.post_id = p.id) as comments from posts p
