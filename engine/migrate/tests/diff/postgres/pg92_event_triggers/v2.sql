create table kept (id integer primary key);
create function log_ddl() returns event_trigger language plpgsql as $$ begin null; end $$;
create event trigger ddl_logger on ddl_command_end execute function log_ddl();
alter event trigger ddl_logger disable;
comment on event trigger ddl_logger is 'logs ddl';
create event trigger table_ddl on ddl_command_start when tag in ('CREATE TABLE', 'ALTER TABLE') execute function log_ddl();
