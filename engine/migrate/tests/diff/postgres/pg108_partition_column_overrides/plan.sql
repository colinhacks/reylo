-- file 1 of 2
create table orders_us partition of orders (
  priority default 10,
  notes not null
) for values in ('us');
alter table orders_asia alter column priority set default 0;
alter table orders_eu add constraint notes_not_null check (notes is not null) not valid;
-- file 2 of 2
alter table orders_eu validate constraint notes_not_null;
alter table orders_eu alter column notes set not null;
alter table orders_eu drop constraint notes_not_null;
