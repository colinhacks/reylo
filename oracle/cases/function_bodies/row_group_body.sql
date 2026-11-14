select lookup_id(c.post_id) as g, count(*) as n from comments c group by lookup_id(c.post_id)
