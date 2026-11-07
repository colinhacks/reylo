create extension postgres_fdw;
create server remote foreign data wrapper postgres_fdw options (host 'localhost', dbname 'a');
create foreign table remote_users (id integer, email text) server remote options (table_name 'users');
