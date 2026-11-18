create table kept (id integer primary key);
create table accounts (id integer primary key, active boolean not null);
create view active_accounts as select id from accounts where active;
alter table accounts enable row level security;
create policy account_access on accounts for select to public using (id in (select id from active_accounts));
