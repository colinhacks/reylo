with recursive `a``b` as (select cast(null as signed) as n), r (n) as (select 1 union all select s.n from r cross join `a``b` as s where r.n = 1) select n from r
