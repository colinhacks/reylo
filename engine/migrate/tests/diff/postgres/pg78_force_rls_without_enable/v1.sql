create table notes (id integer primary key);
create table drafts (id integer primary key);
alter table drafts force row level security;
