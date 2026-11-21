with recursive a (n) as (select 1 union all select n + 1 from a where n < 3), b (m) as (select n from a union all select m + 1 from b where m < 5) select m from b
