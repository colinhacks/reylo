create schema b;
create type b.mood as enum('b');
create type mood as enum('m');
create type b.pair as struct(x enum('m'));
create macro b.f (x enum('m')) as (cast(x as VARCHAR));
create table b.t (id integer, p b.pair, s varchar, primary key (id), check ((b.f(s) is not null)));
