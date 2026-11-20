select c.id, c.name, o.id as order_id, o.total from customers c left join orders o on o.customer_id = c.id
