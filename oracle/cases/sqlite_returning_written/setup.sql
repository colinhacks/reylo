-- A write's RETURNING reads the row as the write left it: a column the statement wrote a non-null
-- value to holds that value. SQLite computes an UPDATE's values before its BEFORE triggers run
-- and loads again only the columns the UPDATE leaves alone, no trigger can assign NEW, and
-- RETURNING reports each row before the AFTER triggers run.
create table t (id integer primary key, note text, at text, n integer) strict;
insert into t values (1, null, null, null), (2, 'b', 'x', 2);
create table src (id integer primary key, note text) strict;
insert into src values (1, null), (3, 'c');
create table trig (id integer primary key, note text, other text);
insert into trig values (1, 'a', 'o');
create trigger trig_before before update on trig begin update trig set note = null, other = null where id = new.id; end;
create trigger trig_after after insert on trig begin update trig set note = null where id = new.id; end;
create view tv as select id, note from t;
create trigger tv_insert instead of insert on tv begin insert into t (id, note) values (new.id, null); end;
