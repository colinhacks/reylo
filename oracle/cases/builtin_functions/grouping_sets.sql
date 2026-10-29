select kind, grouping(kind) as rolled, count(*) as n from events group by rollup (kind)
