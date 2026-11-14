create schema app;
alter default privileges in schema app grant select on tables to dp_reader;
alter default privileges revoke execute on functions from public;
