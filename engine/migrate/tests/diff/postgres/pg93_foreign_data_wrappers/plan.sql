create foreign data wrapper plain_fdw;
alter foreign data wrapper test_fdw options (set debug 'false', add option1 'value1', drop mode);
comment on foreign data wrapper test_fdw is 'a wrapper';
comment on server test_server is 'a server';
