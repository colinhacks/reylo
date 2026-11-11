select l.* from posts p cross join lateral (select p.id as v, p.title as w union all select null, null) l
