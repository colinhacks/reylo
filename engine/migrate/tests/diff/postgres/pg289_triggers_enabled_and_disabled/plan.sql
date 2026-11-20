drop trigger b on t;
drop trigger c on t;
drop trigger d on t;
create trigger b before insert on t for each row execute function f();
alter table t disable trigger b;
create trigger c before insert on t for each row execute function f();
alter table t enable replica trigger c;
create trigger d before insert on t for each row execute function f();
alter table t enable always trigger d;
