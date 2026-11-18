create table orders (id bigint not null, region text not null, priority integer default 0, notes text) partition by list (region);
create table orders_eu partition of orders for values in ('eu');
create table orders_asia partition of orders (priority default 5) for values in ('asia');
