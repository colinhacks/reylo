create type region as enum ('eu', 'us');
create table t (id integer primary key, r region);
alter table t enable row level security;
