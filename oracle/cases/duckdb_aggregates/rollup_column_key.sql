select coalesce(region, 'x') as r, coalesce(sum(total), 0) as s, 7 as k, region is null as missing, count(*) + 1 as c, grouping(region) as g from orders group by rollup (region)
