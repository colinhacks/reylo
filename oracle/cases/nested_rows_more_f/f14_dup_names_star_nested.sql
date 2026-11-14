select p.id, array(select c from (select l.* from comments x cross join lateral (select x.id + 1 as v, x.body as v) l where x.post_id = p.id) c) as arr from posts p
