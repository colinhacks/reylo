create table items (id integer, name text, note text);
create view v_items as select id, name from items;
grant select (id), insert (name) on v_items to diff_viewer;
