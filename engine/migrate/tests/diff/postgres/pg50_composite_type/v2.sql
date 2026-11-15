create type address as (street text, city text, zip varchar(10));
create table shops (id integer primary key, at address);
