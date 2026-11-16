-- An array a DISTINCT aggregate builds holds no two equal elements, one NULL at most, bare or under a coalesce with an empty array, in a column and inside JSON; an element that is an array, which Postgres compares with its bounds, and anything a cast, a set operation or a read through a query changes, says nothing.
create table posts (id int primary key, title text not null);
create table tags (id int primary key, post_id int, name text, score numeric, f float8, at timestamptz);
insert into posts values (1, 'a'), (2, 'b');
insert into tags values
  (1, 1, 'x', 1.0, 0, '2026-01-01 00:00:00+00'),
  (2, 1, 'x', 1.00, -0, '2026-01-01 01:00:00+01'),
  (3, 1, null, null, 'NaN', null),
  (4, 1, null, null, 'NaN', null),
  (5, 2, 'X', 2, 1.5, '2026-01-02 00:00:00+00'),
  (6, null, 'y', 2.0, 1.5, null);
