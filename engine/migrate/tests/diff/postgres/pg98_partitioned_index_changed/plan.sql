-- file 1 of 7
drop index idx_orders_region;
drop index idx_orders_status_old;
-- file 2 of 7
create index concurrently orders_2024_by_status on orders_2024 (status);
-- file 3 of 7
create index concurrently orders_2024_region_idx on orders_2024 using btree (region) with (fillfactor='70');
-- file 4 of 7
create index concurrently orders_2025_by_status on orders_2025 (status);
-- file 5 of 7
create index idx_orders_status_new on orders using btree (status);
-- file 6 of 7
create index concurrently orders_2025_region_idx on orders_2025 using btree (region) with (fillfactor='70');
-- file 7 of 7
create index idx_orders_region on orders using btree (region) with (fillfactor='70');
