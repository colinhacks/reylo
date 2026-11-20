select c.name, o.shipped from customers c left join orders o on o.customer_id = c.id where c.name is not null and o.shipped > '2020-01-01'
