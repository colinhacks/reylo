create algorithm = merge view v as select 1 as x;
create procedure p() comment 'touches nothing' select 1;
create function f(x varchar(10) charset latin1) returns varchar(10) charset latin1 deterministic return x;
create event once_in_a_while on schedule at '2099-01-01 00:00:00' do select 1;
