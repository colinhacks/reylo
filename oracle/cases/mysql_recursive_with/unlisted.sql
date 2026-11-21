with recursive nums as (select 1 as n union all select n + 1 from nums where n < 5) select n from nums
