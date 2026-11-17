select c.id, (select max(o.total) from orders o where o.customer_id = c.id) as biggest, (select count(*) from orders o where o.customer_id = c.id) as orders from customers c
