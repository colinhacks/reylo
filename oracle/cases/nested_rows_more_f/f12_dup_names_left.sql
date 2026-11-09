select l.* from posts p left join lateral (select 1 as v, x.body as v from comments x where x.post_id = p.id) l on true
