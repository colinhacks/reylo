create type mood as enum ('a', 'b', 'c');
     create table p (id integer primary key, name varchar check (name <> ''));
     create table c (id integer primary key, p_id integer references p (id), m varchar check (m::mood is not null), extra integer);
