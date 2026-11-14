select p.*, l.* from posts p cross join lateral (select 1 as title, x.body as title from comments x where x.post_id = p.id) l
