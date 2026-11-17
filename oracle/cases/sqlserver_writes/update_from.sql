update o set o.status = 'paid' output inserted.id, deleted.status from orders o join customers c on c.id = o.customer_id where c.vip = 1
