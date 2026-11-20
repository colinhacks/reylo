-- file 1 of 4
drop index public.pi;
-- file 2 of 4
create index concurrently j_id_idx on j using btree (id);
-- file 3 of 4
create index concurrently new_child on h (id);
-- file 4 of 4
create index pi on p using btree (id);
