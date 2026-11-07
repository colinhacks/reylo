drop function bump;
create function bump(x int) returns int deterministic return x + 2;
