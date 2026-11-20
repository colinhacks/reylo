create type e as enum ('a', 'b', 'c');
create table p (id int not null, c e default 'a') partition by range (id);
create table x (id int not null, c e default 'a');
