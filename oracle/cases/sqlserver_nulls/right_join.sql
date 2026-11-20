select c.email, o.id from customers c right join orders o on o.customer_id = c.id
