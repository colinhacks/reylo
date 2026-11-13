select o.id, x.* from orders o left join lateral (select o.customer, i.sku from items i where i.order_id = o.id order by i.id limit 1) x on true
