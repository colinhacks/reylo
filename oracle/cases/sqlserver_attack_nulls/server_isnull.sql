select isnull(m, cast(null as int)) as a, isnull(nullif(n, 0), m) as b, coalesce(m, null) as c from f
