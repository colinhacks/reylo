select c.id, x.total from customers c cross apply (select top 1 o.total from orders o where o.customer_id = c.id order by o.placed desc) x
