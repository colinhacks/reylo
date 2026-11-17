select case when exists (select 1 from orders where customer_id = :customer_id) then 1 else 0 end as has_orders
