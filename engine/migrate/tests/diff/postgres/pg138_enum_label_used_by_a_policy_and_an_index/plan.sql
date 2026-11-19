-- file 1 of 3
alter type region rename to region__old;
create type region as enum ('eu', 'us', 'ap');
alter table t alter column r type region using r::text::region;
drop type region__old;
-- file 2 of 3
create index concurrently t_ap on t (id) where r = 'ap';
-- file 3 of 3
create policy only_ap on t using (r = 'ap');
