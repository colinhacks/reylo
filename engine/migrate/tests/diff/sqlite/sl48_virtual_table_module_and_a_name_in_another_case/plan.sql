-- loses data: r
-- loses data: Élan
drop table r;
drop table "Élan";
create virtual table r using rtree_i32(id, x0, x1);
create table "élan" (id integer primary key);
