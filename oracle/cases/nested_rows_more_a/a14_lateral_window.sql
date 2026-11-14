select l.prev, l.nxt from posts p cross join lateral (select lag(x.big) over (order by x.id) as prev, lead(x.big + 1) over (order by x.id) as nxt from comments x where x.post_id = p.id) l
