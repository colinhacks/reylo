select l.* from posts p cross join lateral (select x.post_id + 1 as k, count(*) as n from comments x where x.post_id = p.id group by cube (x.post_id)) l
