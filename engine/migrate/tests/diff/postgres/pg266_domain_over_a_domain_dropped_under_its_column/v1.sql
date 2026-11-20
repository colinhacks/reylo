create domain a as text;
create domain b as a constraint b_ck check (length(value) > 0);
create table t (id int, n b);
