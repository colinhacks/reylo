select customer, sum(total) filter (where total > 100) as big from orders group by customer
