create table new_settings (key text primary key, value text) without rowid;
insert into new_settings (key, value) select key, value from settings;
drop table settings;
alter table new_settings rename to settings;
