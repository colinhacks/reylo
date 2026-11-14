create type address as (street text, city text);
create table shops (id integer primary key, at address);
