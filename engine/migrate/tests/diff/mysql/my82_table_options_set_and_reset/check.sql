insert into invoices (total) values (1);
set @first = (select if(max(id) = 1000, 1, (select 1 union select 2)) from invoices);
