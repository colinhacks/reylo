with recursive r (n, m) as (select 1, 1 union all select n + 1, nullif(m, 1) from r where n < 3) select n, m from r
