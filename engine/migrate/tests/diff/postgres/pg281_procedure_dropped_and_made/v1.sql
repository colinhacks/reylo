create table salary (emp int, amount int);
create procedure raise(p_emp int, p_amount int) language sql as $$ update salary set amount = p_amount where emp = p_emp $$;
