-- file 1 of 4
create index concurrently p1_a_idx on p1 using btree (a);
-- file 2 of 4
alter table p attach partition p1 for values from (0) to (10);
alter table p attach partition p2 for values from (10) to (20);
-- file 3 of 4
create index concurrently q1_a_idx on q1 using btree (a);
-- file 4 of 4
alter table q attach partition q1 for values from (0) to (10);
