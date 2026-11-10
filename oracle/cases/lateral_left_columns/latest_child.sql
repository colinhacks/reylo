select o.id, x.customer, x.sku, to_jsonb(x) as item from orders o left join lateral (select o.customer, i.sku from items i where i.order_id = o.id order by i.id limit 1) x on true
