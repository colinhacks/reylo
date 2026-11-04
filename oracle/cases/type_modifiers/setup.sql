-- Type modifiers: the length, precision, tick or fields a column declares, as RowDescription and
-- the catalog report them, through a cast, a domain, a composite type, an array, a view and an
-- interval's fields. The rows hold each type's edges: the longest text, the most digits, the finest
-- tick.
create domain email as varchar(255) check (value like '%@%');
create type address as (street text, zip varchar(10));
create table typed (
  id int primary key,
  name varchar(20) not null,
  code char(5) not null,
  flag char,
  price numeric(10,2),
  rounded numeric(5,-2),
  whole numeric(5),
  at_time time(3),
  at_zone timetz(0),
  at timestamp(3),
  at_tz timestamptz(0),
  span interval,
  months interval year to month,
  days interval day,
  hours interval hour,
  minutes interval hour to minute,
  tenths interval second(1),
  flags bit(7),
  bits varbit(5),
  tags varchar(8)[],
  mail email,
  home address
);
insert into typed values
  (1, 'ada', 'ab', 'x', 99999999.99, 9999900, 99999, '23:59:59.999', '12:00:00+05', '2026-01-01 12:00:00.123', '2026-01-01 12:00:00+00', '1 year 2 mons 3 days 04:05:06.789', '1 year 2 months', '3 days 04:05', '1 day 04:05:06', '04:05:06', '00:00:01.25', B'1010101', B'101', '{alpha,beta}', 'ada@example.com', ('1 Main St', '02134')),
  (2, repeat('z', 20), 'abcde', 'y', -0.01, -100, 0, '00:00:00', '00:00:00-08', '2026-12-31 23:59:59.999', '2026-12-31 23:59:59+00', '-1 day', '-3 years', '-1 day -02:00', '25 hours', '00:00:59', '0.1 second', B'0000000', B'', '{}', 'b@c.d', null),
  (3, '', 'q', ' ', 'NaN', null, null, null, null, null, null, null, null, null, null, null, null, null, null, '{abcdefgh,NULL}', null, null);
create view typed_names as select name, price, tags from typed;
