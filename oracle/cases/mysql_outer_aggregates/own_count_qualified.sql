select p.id, (select count(c.id) from comments c where c.post_id = p.id) as n from posts p
