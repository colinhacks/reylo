create domain d as int;
alter domain d add constraint positive check (value > 0) not valid;
create table t (n d);
