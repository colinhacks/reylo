create table kept (id integer primary key);
alter default privileges in schema public grant all on tables to diff_anon, diff_auth;
alter default privileges grant execute on functions to diff_anon;
create table test (id integer primary key, data text);
revoke all on test from diff_anon;
create function f() returns integer language sql as 'select 1';
revoke execute on function f() from diff_anon;
