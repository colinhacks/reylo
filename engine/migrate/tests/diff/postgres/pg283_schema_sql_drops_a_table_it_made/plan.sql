set check_function_bodies = off;
create procedure "Add User"(name text) language sql as $$ insert into users default values $$;
reset check_function_bodies;
