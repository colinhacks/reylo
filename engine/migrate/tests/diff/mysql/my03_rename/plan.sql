-- users.full_name is renamed name, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users rename column full_name to name;
