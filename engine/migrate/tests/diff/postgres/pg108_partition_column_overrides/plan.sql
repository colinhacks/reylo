create table orders_us partition of orders (
  priority default 10,
  notes not null
) for values in ('us');
alter table orders_asia alter column priority set default 0;
alter table orders_eu alter column notes set not null;
