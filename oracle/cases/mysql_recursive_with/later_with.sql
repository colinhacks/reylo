with recursive nums (n) as (select 1 union all select n + 1 from nums where n < 3), doubled as (select n * 2 as d from nums) select d from doubled
