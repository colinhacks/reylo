-- Built-in functions and operators that answer a value for every non-null input, against the
-- forms of them that do not: an infinite timestamp, an empty range, a pattern that does not match,
-- a NULL among the items compared, a frame without the current row, a set-returning call beside
-- another one.
create type mood as enum ('sad', 'ok', 'happy');
create table events (
  id bigint primary key,
  kind text not null,
  note text,
  amount numeric not null,
  at timestamptz not null,
  local_at timestamp not null,
  day date not null,
  clock time not null,
  wait interval not null,
  span tstzrange not null,
  spans int4multirange not null,
  tags text[] not null,
  mood mood not null,
  n int not null
);
