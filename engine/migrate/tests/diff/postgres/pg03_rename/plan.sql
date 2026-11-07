-- users.full_name is renamed name, keeping its rows. If they are two columns, write instead:
--   alter table users drop column full_name;
--   alter table users add column name text not null;
alter table users rename column full_name to name;
