select customer_id, status, sum(total) as total from orders group by rollup (customer_id, status)
