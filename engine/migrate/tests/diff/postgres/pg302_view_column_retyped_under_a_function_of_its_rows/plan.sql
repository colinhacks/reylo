set check_function_bodies = off;
drop function item_view_all();
drop view item_view;
create view item_view as select id, due from items;
create function item_view_all() returns setof item_view language sql stable as $$ select * from item_view $$;
create function items_due_between(p_start date, p_end date) returns setof item_view language sql stable as $$ select id, due from items where due between p_start and p_end $$;
revoke execute on function items_due_between(date,date) from public;
reset check_function_bodies;
