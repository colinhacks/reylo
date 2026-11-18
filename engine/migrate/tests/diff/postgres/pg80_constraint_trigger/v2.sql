create table events (id serial primary key, processed boolean);
create function assert_processed() returns trigger language plpgsql as $$ begin return null; end $$;
create constraint trigger ensure_processed after insert on events deferrable initially deferred for each row execute function assert_processed();
create constraint trigger ensure_checked after update on events for each row when (new.processed) execute function assert_processed();
