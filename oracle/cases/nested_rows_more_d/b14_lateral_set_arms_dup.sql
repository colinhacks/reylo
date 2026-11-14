select l.* from posts p cross join lateral (select p.id as v, null::text as v union all select p.id, 'z') l
