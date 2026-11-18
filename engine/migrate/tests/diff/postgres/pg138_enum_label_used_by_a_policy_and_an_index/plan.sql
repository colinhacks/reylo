alter type region rename to region__old;
create type region as enum ('eu', 'us', 'ap');
alter table t alter column r type region using r::text::region;
drop type region__old;
create index t_ap on t (id) where r = 'ap';
create policy only_ap on t using (r = 'ap');
