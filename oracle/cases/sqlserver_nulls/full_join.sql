select c.id as customer, o.id as order_id from customers c full join orders o on o.customer_id = c.id
