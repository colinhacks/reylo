select p.title, c.id from posts p cross join lateral alias_named_p(p.id) c
