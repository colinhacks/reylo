select customer, region, sum(total) as revenue from orders group by rollup (customer, region)
