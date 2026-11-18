create table kept (id integer primary key);
create domain email_address as text;
grant usage on type email_address to diff_type_user;
create type mood as enum ('a', 'b');
revoke usage on type mood from public;
