create type e as enum ('a', 'b', 'c');
create table p (id int not null, c e) partition by range (id);
create table x (id int not null, c e);
