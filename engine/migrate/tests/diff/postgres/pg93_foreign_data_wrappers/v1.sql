create table kept (id integer primary key);
create foreign data wrapper test_fdw options (debug 'true', mode 'a');
create server test_server foreign data wrapper test_fdw options (host 'a');
