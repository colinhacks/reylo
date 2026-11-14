select l.s from posts p cross join lateral (select x.post_id, sum(x.big) filter (where x.id > 100) as s from comments x where x.post_id = p.id group by x.post_id) l
