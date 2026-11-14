create macro zz (x) as ((x > 0));
create table a_t (a integer, check (a.zz()));
