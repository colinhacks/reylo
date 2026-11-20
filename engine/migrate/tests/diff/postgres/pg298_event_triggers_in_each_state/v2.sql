create function log_ddl() returns event_trigger language plpgsql as $$ begin null; end $$;
create event trigger e1 on ddl_command_end execute function log_ddl();
create event trigger e2 on ddl_command_end execute function log_ddl();
create event trigger e3 on ddl_command_end execute function log_ddl();
alter event trigger e1 enable replica;
alter event trigger e2 enable always;
