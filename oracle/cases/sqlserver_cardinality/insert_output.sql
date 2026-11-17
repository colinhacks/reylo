insert into orders (customer_id, status, total, placed) output inserted.id, inserted.placed values (:customer_id, 'open', :total, sysdatetimeoffset())
