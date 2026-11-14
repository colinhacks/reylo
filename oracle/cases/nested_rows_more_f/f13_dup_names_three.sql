select l.* from posts p cross join lateral (select x.body as v, 1 as v, x.body as v from comments x where x.post_id = p.id) l
