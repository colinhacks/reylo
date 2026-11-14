create table docs (id integer primary key, body text);
alter table docs enable row level security;
alter table docs force row level security;
