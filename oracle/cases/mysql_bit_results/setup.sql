-- An IF or a CASE with a BIT among its results: the server types it by its other results, DECIMAL
-- beside a number, and returns the BIT's own bytes in it; where every result is a BIT it prepares
-- a BIT and may send a number. Such a column is of no known type. A coalesce and an ifnull return
-- digits, and an IF or a CASE with no BIT keeps its type. Given BITs alone, `coalesce`, `ifnull`,
-- `greatest`, `least` and `any_value` are prepared as a BIT and return the decimal digits of its
-- number, which are of no known type either; `max` returns the BIT's bytes.
create table flags (id int primary key, n tinyint not null, enabled bit(1) not null, mask bit(64) not null);
-- Such a call in a view, a derived table or a WITH query sends digits through it too, under a BIT
-- type that names no schema, where `max` there sends the BIT's bytes: read as a column, by name or
-- through `*`, a BIT computed there is of no known type, and the table's own column stays a BIT.
create view flag_digits as select id, coalesce(mask, mask) as digits, mask from flags;
