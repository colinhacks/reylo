create type mood as enum ('a', 'b'); create table p (id integer primary key, m mood); create table c (id integer primary key, p_id integer references p (id));
