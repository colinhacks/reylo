-- users.code is renamed sku, keeping its rows. If they are two columns, write instead:
--   alter table users drop column code;
--   alter table users add column sku text;
alter table users rename column code to sku;
