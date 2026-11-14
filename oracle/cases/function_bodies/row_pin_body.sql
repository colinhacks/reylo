select c.id from comments c where c.id = lookup_id(c.post_id)
