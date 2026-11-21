with recursive `a``b` as (select cast(null as signed) as n), r (n) as (select n from `a``b` union all select n from r where n < 0) select n from r
