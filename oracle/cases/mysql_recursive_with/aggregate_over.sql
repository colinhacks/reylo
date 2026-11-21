with recursive nums (n) as (select 1 union all select n + 1 from nums where n < 3) select max(n) as top, count(n) as c from nums
