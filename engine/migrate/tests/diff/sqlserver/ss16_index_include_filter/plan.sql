create unique nonclustered index orders_customer_id on orders (customer, id);
create nonclustered index orders_open on orders (customer desc) include (total) where ([status]='open');
