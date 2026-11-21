with recursive r (n, m) as (select 1, 1 union all select m, null from r where n < 3 and n is not null) select n, m from r
