select customer_id, count(*) as n, sum(total) as total, max(shipped) as last_shipped from orders group by customer_id
