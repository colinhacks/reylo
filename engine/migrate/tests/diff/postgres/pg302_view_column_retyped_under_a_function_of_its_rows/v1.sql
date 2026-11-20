create table items (id int, due date);
create view item_view as select id, to_char(due, 'MM/DD/YYYY') as due from items;
create function item_view_all() returns setof item_view language sql stable as $$ select * from item_view $$;
