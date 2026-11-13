-- An IF or a CASE with a BIT among its results: the server types it by its other results, DECIMAL
-- beside a number, and returns the BIT's own bytes in it; where every result is a BIT it prepares
-- a BIT and may send a number. Such a column is of no known type. A coalesce and an ifnull return
-- digits, and an IF or a CASE with no BIT keeps its type.
create table flags (id int primary key, n tinyint not null, enabled bit(1) not null, mask bit(64) not null);
