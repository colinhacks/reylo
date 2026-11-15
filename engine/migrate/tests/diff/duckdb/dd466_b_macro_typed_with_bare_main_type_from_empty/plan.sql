create schema b;
create type b.mood as enum('b');
create type mood as enum('m');
create macro b.f (x enum('m')) as (cast(x as VARCHAR));
create table b.t (id integer, s varchar, primary key (id), check ((b.f(s) is not null)));
