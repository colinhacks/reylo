create table kept (id integer primary key);
create foreign data wrapper test_fdw options (debug 'false', option1 'value1');
comment on foreign data wrapper test_fdw is 'a wrapper';
create server test_server foreign data wrapper test_fdw options (host 'a');
comment on server test_server is 'a server';
create foreign data wrapper plain_fdw;
