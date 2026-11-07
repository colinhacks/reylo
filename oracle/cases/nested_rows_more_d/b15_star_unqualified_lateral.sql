select * from posts p cross join lateral (select max(x.big) + 1 as more from comments x where x.post_id = p.id) l
