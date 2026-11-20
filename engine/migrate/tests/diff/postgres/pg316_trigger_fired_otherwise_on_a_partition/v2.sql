create table p (id int, a int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
create function tf() returns trigger language plpgsql as $$ begin return new; end $$;
create trigger tg before insert on p for each row execute function tf();
alter table p1 disable trigger tg;
create table q (id int, a int) partition by range (id);
create table q1 partition of q for values from (0) to (10);
create trigger tq before insert on q for each row execute function tf();
alter table q enable replica trigger tq;
