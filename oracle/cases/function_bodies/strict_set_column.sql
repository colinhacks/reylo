select s.a from comments c cross join lateral strict_set(length(c.note)::int8) s
