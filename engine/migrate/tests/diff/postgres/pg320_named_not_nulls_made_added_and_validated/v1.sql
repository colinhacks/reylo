create table u (id int);
create table y (id int);
create table z (id int primary key, a int);
create table v (a int);
alter table v add constraint v_a_old not null a not valid;
create table k (id int, a int, constraint k_a_not_null check (true));
create table h (a int not null);
create type ty as (a int);
create table typed (id int);
