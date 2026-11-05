-- A column of a STRICT table holds its declared type alone, and so does a rowid in any table. A generated column of a STRICT table keeps its affinity's union: SQLite checks what it computes only from 3.51, and an older SQLite stores anything there. ANY holds every storage class, and outside a STRICT table every other column holds what its affinity stores: an INTEGER PRIMARY KEY DESC, a key in a WITHOUT ROWID table and an INT PRIMARY KEY are no rowids.
create table s (id integer primary key, i int not null, n integer, r real, t text, b blob, a any, g integer generated always as (i * 2) virtual, h text generated always as (t || 'x') stored) strict;
create table w (k text primary key, v integer) strict, without rowid;
create table loose (id integer primary key, n integer, r real, t text, b blob);
create table desc_key (a integer primary key desc);
create table int_key (a int primary key);
create table no_rowid (a integer primary key, b text) without rowid;
create view strict_view as select id, i, t from s;
create view loose_view as select id, n from loose;
