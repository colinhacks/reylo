select l.* from posts p left join lateral (select x.big + 1 as more, x.id from comments x where x.post_id = p.id) l on true
