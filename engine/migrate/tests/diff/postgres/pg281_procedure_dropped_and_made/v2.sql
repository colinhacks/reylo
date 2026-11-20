create table salary (emp int, amount int);
create procedure "Add salary"(p_emp int) language sql as $$ insert into salary values (p_emp, 0) $$;
