create table orders (id bigint not null, region text not null, priority integer default 0, notes text) partition by list (region);
create table orders_eu partition of orders (notes not null) for values in ('eu');
create table orders_asia partition of orders for values in ('asia');
create table orders_us partition of orders (priority default 10, notes not null) for values in ('us');
