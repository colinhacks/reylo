select l.* from posts p cross join lateral (select x.body as b, 1 as v from comments x where x.post_id = p.id) l(v)
