-- `%` casts both operands to INTEGER before the remainder
-- (https://sqlite.org/lang_expr.html#binaryops). A fractional literal that truncates to 0 is a
-- zero divisor and returns NULL on every row; `/` keeps the float and stays non-null.
create table items (id integer primary key, qty integer not null);
insert into items values (1, 5), (2, 7), (3, 0);
