with recursive r (n, m) as (select 1, 1 union all select n + 1, m from r where n < 3) select 5 as n union all (with r (n, m) as (select 1, null) select n from r) union all select m from r
