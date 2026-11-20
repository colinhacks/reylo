create domain a as text constraint a_ck check (length(value) > 0);
create table t (id int, n a);
