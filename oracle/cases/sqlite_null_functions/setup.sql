-- What SQLite answers NULL for from non-null arguments: a date and time function for a time value
-- it cannot read, one out of its range or a modifier that fails (the seeded text, 'a' and '', is no
-- date; -1 and 1e308 are no julian day or unix time), printf for an empty format, substr for an
-- empty blob, a JSON path that finds nothing, iif with no ELSE, and % by a literal under 1, since %
-- casts its divisor to INTEGER (0.5 is 0) where / keeps it. This oracle's SQLite has no math
-- functions (sqlite_functions), so their domains are pinned by the engine's tests alone.
create table o (id integer primary key, placed_at text not null, n integer not null, x real not null, b blob not null, fmt text not null) strict;
