select p.id, array(select c from comments c where c.post_id = p.id) as comments from posts p
