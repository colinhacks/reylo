-- file 1 of 3
drop index j;
alter index i alter column 2 set statistics 1000;
alter index i alter column 3 set statistics 100;
-- file 2 of 3
create index concurrently j on t using btree (((a * 3)));
-- file 3 of 3
alter index j alter column 1 set statistics 500;
