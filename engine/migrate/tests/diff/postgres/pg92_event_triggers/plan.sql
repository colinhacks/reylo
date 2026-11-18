drop event trigger ddl_dropped;
alter event trigger ddl_logger disable;
create event trigger table_ddl on ddl_command_start when tag in ('ALTER TABLE', 'CREATE TABLE') execute function log_ddl();
comment on event trigger ddl_logger is 'logs ddl';
