alter default privileges in schema app revoke insert on tables from dp_reader;
alter default privileges in schema app grant select on tables to dp_reader;
alter default privileges revoke execute on functions from public;
