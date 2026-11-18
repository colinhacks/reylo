create table orders (order_id integer not null, created_on date not null, status text, region text) partition by range (created_on);
create table orders_2024 partition of orders for values from ('2024-01-01') to ('2025-01-01');
create table orders_2025 partition of orders for values from ('2025-01-01') to ('2026-01-01');
create index idx_orders_status_new on only orders (status);
create index orders_2024_by_status on orders_2024 (status);
alter index idx_orders_status_new attach partition orders_2024_by_status;
create index orders_2025_by_status on orders_2025 (status);
alter index idx_orders_status_new attach partition orders_2025_by_status;
create index idx_orders_region on orders (region) with (fillfactor = 70);
