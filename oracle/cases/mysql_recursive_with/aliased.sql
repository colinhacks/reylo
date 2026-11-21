with recursive nums (n) as (select 1 union all select n + 1 from nums where n < 5) select x.n, x.n * 2 as twice from nums as x
