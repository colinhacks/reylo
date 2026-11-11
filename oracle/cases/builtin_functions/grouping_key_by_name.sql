-- A grouping key is NULL in the rows a grouping set leaves it out of, even a constant.
select 1 as one, 'x' as label, count(*) as n from events group by rollup (one)
