create table kept (id integer primary key);
create function log_ddl() returns event_trigger language plpgsql as $$ begin null; end $$;
create event trigger ddl_logger on ddl_command_end execute function log_ddl();
create event trigger ddl_dropped on sql_drop execute function log_ddl();
