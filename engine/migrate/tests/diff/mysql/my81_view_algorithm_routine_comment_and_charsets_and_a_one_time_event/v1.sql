create view v as select 1 as x;
create procedure p() select 1;
create function f(x varchar(10)) returns varchar(10) deterministic return x;
