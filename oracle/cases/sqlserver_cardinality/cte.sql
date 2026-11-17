with recent as (select customer_id, max(placed) as last from orders group by customer_id) select c.email, r.last from customers c join recent r on r.customer_id = c.id
