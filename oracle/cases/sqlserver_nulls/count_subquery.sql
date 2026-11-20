select c.id, (select count(*) from orders o where o.customer_id = c.id) as orders, (select count_big(*) from lines) as lines, (select top 1 count(*) from orders) as topped from customers c
