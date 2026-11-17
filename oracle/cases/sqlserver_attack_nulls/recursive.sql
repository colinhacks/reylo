with r (k, z) as (select 1, 1 union all select k + 1, case when k > 2 then null else z end from r where k < 5) select k, z from r
