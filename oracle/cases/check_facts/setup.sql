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
-- A cast that changes its constant, and a real compared with a double precision, can pass a value past the constant the CHECK prints, so neither gives a bound; one that keeps its constant does.
create table rounding (
  id int primary key,
  to_int numeric check (to_int <= 2.5::int),
  to_scale numeric check (to_scale <= 2.55::numeric(3,1)),
  past_float float8 check (past_float >= 0.30000000000000001),
  through_real float8 check (through_real <= 0.1::real::float8),
  single float4 check (single > 0.1),
  below_single float4 check (below_single < 0.7),
  kept_int numeric check (kept_int <= 2::int),
  kept_float float8 check (kept_float >= 0.3),
  kept_single float4 check (kept_single >= 0.5),
  kept_real float4 check (kept_real >= 0.1::real),
  no_whole int check (no_whole > 5 and no_whole < 6)
);
insert into rounding (id, to_int, to_scale, past_float, through_real, single, below_single, kept_int, kept_float, kept_single, kept_real) values
  (1, 2.9, 2.58, 0.3, 0.1000000012, 0.1, 0.7, 2, 0.3, 0.5, 0.1),
  (2, null, null, null, null, null, null, null, null, null, null);
