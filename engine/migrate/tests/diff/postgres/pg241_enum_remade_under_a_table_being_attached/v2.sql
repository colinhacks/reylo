create type e as enum ('a', 'c');
create table p (id int not null, c e default 'a') partition by range (id);
create table x partition of p for values from (1) to (4);
