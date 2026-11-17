create table orders (id int not null primary key, customer int not null, status varchar(10) not null, total decimal(10, 2) not null);
create index orders_open on orders (customer desc) include (total) where status = 'open';
create unique index orders_customer_id on orders (customer, id);
