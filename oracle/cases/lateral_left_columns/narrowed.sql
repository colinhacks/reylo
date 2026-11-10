select o.id, x.customer from orders o left join lateral (select o.customer from items i where i.order_id = o.id limit 1) x on true where x.customer is not null
