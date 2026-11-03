-- A CHECK that compares a float or numeric column with constants keeps out the non-finite values
-- it is false of. Postgres sorts NaN after every number and holds it equal to itself, so x < c
-- keeps out NaN and infinity, x >= c -infinity alone, and x <> 'NaN' NaN alone; the type has one
-- flag for both infinities. A bare read, a column a JSON constructor reads and a stored row
-- inside JSON hold no value a CHECK keeps out; an expression over the column, or a CHECK added
-- NOT VALID, says nothing.
create domain finite_score as float8 check (value > -1e9 and value < 1e9);
create table prices (
  id int primary key,
  total numeric(10,2) not null check (total >= 0 and total < 1e8),
  rate float8 check (rate <> 'NaN'),
  ratio float8 check (ratio between 0 and 1),
  amount numeric check (amount >= 0),
  score finite_score,
  open_ended numeric check (open_ended not in ('NaN', 'Infinity', '-Infinity')),
  raw float8
);
create table legacy_prices (id int primary key, v float8);
insert into legacy_prices values (1, 'NaN');
alter table legacy_prices add constraint legacy_prices_v_check check (v <> 'NaN') not valid;
