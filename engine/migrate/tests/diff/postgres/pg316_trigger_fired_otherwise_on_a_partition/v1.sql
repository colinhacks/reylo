create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on p for each row execute function tf();
