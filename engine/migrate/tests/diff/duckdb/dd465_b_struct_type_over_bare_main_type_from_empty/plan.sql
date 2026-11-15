create schema b;
create type b.mood as enum('b');
create type mood as enum('m');
create type b.pair as struct(x enum('m'));
create table b.t (id integer, p b.pair, primary key (id));
