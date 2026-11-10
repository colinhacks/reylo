select l.k, l.b from posts p cross join lateral (select x.post_id + p.id as k, x.big as b from comments x where x.post_id = p.id group by grouping sets ((x.post_id, x.big), ())) l
