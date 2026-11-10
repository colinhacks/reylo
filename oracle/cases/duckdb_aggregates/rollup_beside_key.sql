select customer = 'a' as is_a, region, count(*) as n from orders group by customer = 'a', rollup (region)
