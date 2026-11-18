drop trigger ensure_processed on events;
create constraint trigger ensure_checked after update on events for each row when (new.processed) execute function assert_processed();
create constraint trigger ensure_processed after insert on events deferrable initially deferred for each row execute function assert_processed();
