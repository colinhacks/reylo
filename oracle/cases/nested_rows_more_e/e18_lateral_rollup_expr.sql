select l.k from posts p cross join lateral (select x.post_id + 1 as k from comments x where x.post_id = p.id group by rollup (x.post_id)) l
