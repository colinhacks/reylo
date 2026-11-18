create table t (id integer primary key);
revoke all on table t from current_user;
create table u (id integer primary key);
create sequence s;
revoke usage on sequence s from current_user;
create table fresh (id integer);
revoke insert, update on table fresh from current_user;
