create table kept (id integer primary key);
alter default privileges in schema public grant all on tables to diff_anon, diff_auth;
alter default privileges grant execute on functions to diff_anon;
