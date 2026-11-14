select e.x2 from comments c cross join lateral echo(length(c.note)::int8) e
