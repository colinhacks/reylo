create table kept (id integer primary key);
create schema s;
create sequence s.q;
revoke usage on sequence s.q from current_user;
revoke usage on schema s from current_user;
