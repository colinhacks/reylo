with recursive r (n, m) as (select 1, 1 union all select n + 1, m from r where n < 3 union all select n + 10, null from r where n = 1) select n, m from r
