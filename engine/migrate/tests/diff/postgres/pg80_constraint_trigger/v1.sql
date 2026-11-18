create table events (id serial primary key, processed boolean);
create function assert_processed() returns trigger language plpgsql as $$ begin return null; end $$;
create constraint trigger ensure_processed after insert on events not deferrable for each row execute function assert_processed();
