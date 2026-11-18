create table items (id integer, name text, note text);
create view v_items as select name, note from items;
grant insert (name), select (note) on v_items to diff_viewer;
