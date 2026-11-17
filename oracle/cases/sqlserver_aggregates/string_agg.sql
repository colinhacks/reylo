select customer_id, string_agg(status, ',') as statuses from orders group by customer_id
