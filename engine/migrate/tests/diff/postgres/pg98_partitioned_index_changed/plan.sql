drop index idx_orders_region;
drop index idx_orders_status_old;
create index orders_2024_by_status on orders_2024 (status);
create index orders_2024_region_idx on orders_2024 using btree (region) with (fillfactor='70');
create index orders_2025_by_status on orders_2025 (status);
create index idx_orders_status_new on orders using btree (status);
create index orders_2025_region_idx on orders_2025 using btree (region) with (fillfactor='70');
create index idx_orders_region on orders using btree (region) with (fillfactor='70');
