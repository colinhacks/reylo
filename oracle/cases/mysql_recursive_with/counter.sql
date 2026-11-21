with recursive nums (n) as (select 1 union all select n + 1 from nums where n < 5) select n from nums
