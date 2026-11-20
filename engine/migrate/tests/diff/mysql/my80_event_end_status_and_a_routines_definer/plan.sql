drop procedure p;
create definer = `definer_owner`@`%` procedure p() select 1;
drop event e;
create event e on schedule every 1 day ends '2099-01-01 00:00:00' disable do select 1;
