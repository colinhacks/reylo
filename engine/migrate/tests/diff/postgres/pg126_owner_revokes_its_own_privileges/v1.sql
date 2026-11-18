create table t (id integer primary key);
create table u (id integer primary key);
create sequence s;
revoke select on table u from current_user;
