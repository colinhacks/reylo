set check_function_bodies = off;
drop procedure raise(integer,integer);
create procedure "Add salary"(p_emp int) language sql as $$ insert into salary values (p_emp, 0) $$;
reset check_function_bodies;
