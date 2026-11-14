select coalesce(region, 'x') as region, count(*) as n from orders group by rollup (region)
