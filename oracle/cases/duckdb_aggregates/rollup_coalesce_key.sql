select coalesce(region, 'n/a') as r, sum(total) as s from orders group by rollup (r)
