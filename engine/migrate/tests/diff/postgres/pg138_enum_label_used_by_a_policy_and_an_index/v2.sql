create type region as enum ('eu', 'us', 'ap');
create table t (id integer primary key, r region);
alter table t enable row level security;
create policy only_ap on t using (r = 'ap');
create index t_ap on t (id) where r = 'ap';
