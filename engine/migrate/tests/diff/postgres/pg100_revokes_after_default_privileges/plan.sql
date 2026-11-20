set check_function_bodies = off;
create table test (
  id integer not null,
  data text,
  constraint test_pkey primary key (id)
);
create function f() returns integer language sql as 'select 1';
revoke delete on table test from diff_anon;
revoke execute on function f() from diff_anon;
revoke insert on table test from diff_anon;
revoke maintain on table test from diff_anon;
revoke references on table test from diff_anon;
revoke select on table test from diff_anon;
revoke trigger on table test from diff_anon;
revoke truncate on table test from diff_anon;
revoke update on table test from diff_anon;
reset check_function_bodies;
