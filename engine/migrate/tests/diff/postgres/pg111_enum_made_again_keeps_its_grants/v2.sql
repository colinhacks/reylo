create type e as enum ('a', 'b');
revoke usage on type e from public;
grant usage on type e to diff_enum_user;
comment on type e is 'kept';
create table t (id integer primary key, v e default 'b');
