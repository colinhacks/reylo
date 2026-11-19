create type mood as enum ('a', 'b');
create domain d as mood;
create table t (id integer primary key, c d);
alter table t enable row level security;
create policy p on t using (c is not null);
