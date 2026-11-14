create table docs (id integer primary key, owner_name text not null, body text);
alter table docs enable row level security;
create policy docs_read on docs for select to reader, writer using (owner_name = current_user or body is null);
