-- What a CHECK holds a value to beyond its type: a number's bounds, its divisor, and a string's pattern, on a column and on a domain, where the column is read as it is, and on a parameter written whole to it. An expression over the column takes none of it, nor does a test that does not bind every row.
create domain email as varchar(255) check (value like '%@%');
create domain percent as int check (value between 0 and 100);
create table items (
  id int primary key,
  age int check (age between 0 and 150),
  qty int not null check (qty % 5 = 0),
  pos int check (pos > 0),
  small smallint check (small >= -40000),
  price numeric(10,2) check (price >= 0),
  rate numeric check (rate > 0 and rate < 1),
  half numeric(6,1) check (half % 0.5 = 0),
  ratio float8 check (ratio >= 0 and ratio <= 1),
  code text check (code ~ '^[a-z]+$'),
  slug varchar(20) check (slug like 'a%'),
  name text check (name ilike 'x%'),
  note text check (note ~* 'abc'),
  pad char(3) check (pad like 'a%'),
  maybe int check (maybe is null or maybe > 10),
  either int check (either > 10 or either < 0),
  flipped int check (not (flipped < 0)),
  beside int check (beside > 0 and id > 0),
  mail email,
  pct percent
);
insert into items (id, age, qty, pos, small, price, rate, half, ratio, code, slug, name, note, pad, maybe, either, flipped, beside, mail, pct) values
  (1, 0, 5, 1, -32768, 0, 0.5, 0.5, 0, 'abc', 'a', 'x', 'abc', 'a', 11, 11, 0, 1, 'a@b', 0),
  (2, 150, 10, 2147483647, 32767, 99999999.99, 0.999, 99999.5, 1, 'z', 'a%', 'XYZ', 'xABCx', 'ab', null, -1, 5, 2, '@', 100),
  (3, null, 0, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null, null);
create view item_view as select age, price, slug from items;
