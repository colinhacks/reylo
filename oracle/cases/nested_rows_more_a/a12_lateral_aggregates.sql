select p.id, l.s, l.more from posts p cross join lateral (select sum(x.big) as s, max(x.big) + 1 as more from comments x where x.post_id = p.id) l
