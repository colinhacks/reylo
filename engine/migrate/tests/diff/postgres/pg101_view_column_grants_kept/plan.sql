drop view v_items;
create view v_items as select name, note from items;
grant insert (name) on table v_items to diff_viewer;
grant select (note) on table v_items to diff_viewer;
