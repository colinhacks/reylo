create extension postgres_fdw;
create server remote foreign data wrapper postgres_fdw options (host 'localhost', dbname 'b');
create foreign table remote_users (id integer, email text) server remote options (schema_name 'public', table_name 'people');
