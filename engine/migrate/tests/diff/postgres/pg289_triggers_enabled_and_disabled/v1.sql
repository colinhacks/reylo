create table t (id int);
create function f() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger a before insert on t for each row execute function f();
create trigger b before insert on t for each row execute function f();
create trigger c before insert on t for each row execute function f();
create trigger d before insert on t for each row execute function f();
