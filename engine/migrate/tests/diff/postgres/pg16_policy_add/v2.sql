create table users (id integer primary key, full_name text not null, n integer not null, code text);
alter table users enable row level security;
create policy users_self on users for select using (id = 1);
