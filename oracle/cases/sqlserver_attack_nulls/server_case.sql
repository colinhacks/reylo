select case when n > 0 then n end as a, iif(m is null, null, n) as b from f
