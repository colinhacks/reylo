-- users.code is renamed sku, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table users rename column code to sku;
