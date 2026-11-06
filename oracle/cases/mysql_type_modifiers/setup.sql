-- Type modifiers: the length, the precision and scale, and the fractional seconds a table's column
-- declares, as the prepare reports them, in each character set it may store; kept through a
-- derived table, a WITH query, an outer join and a JSON document, and not read through a view, a
-- union, an expression, JSON_TABLE or information_schema's own views, whose reported lengths may
-- be display widths. The seeding
-- fills each column to its longest value in characters of one, two and four bytes, and each time
-- with six fractional digits, which a column of fewer rounds to its own.
create table sized (
  id int primary key,
  name varchar(20) not null,
  code char(5),
  legacy varchar(10) character set latin1,
  padded char(4) character set latin1,
  narrow varchar(8) character set utf8mb3,
  wide varchar(6) character set utf16,
  note tinytext,
  body text,
  old_body text character set latin1,
  narrow_note tinytext character set utf8mb3,
  wide_note tinytext character set utf16,
  raw binary(3),
  bits varbinary(8),
  small_blob tinyblob,
  price decimal(10,2),
  huge decimal(65,30),
  positive decimal(6,3) unsigned,
  at datetime,
  at_ms datetime(3),
  at_us datetime(6),
  stamp timestamp(2) null,
  span time,
  span_ms time(4)
);
-- A DECIMAL of no whole digits, apart, since the seeding's first value, 1, does not fit it.
create table parts (id int primary key, part decimal(5,5) not null);
create view sized_view as select name, concat(name, name) as twice, price, at_ms from sized;
