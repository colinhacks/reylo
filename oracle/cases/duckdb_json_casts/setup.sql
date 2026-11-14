-- JSON's `null` is a value, which a NOT NULL json column holds. Its cast to any type but text or
-- JSON is NULL, so such a cast is nullable, through coalesce, an aggregate, a subquery and a type
-- that names JSON too. A cast of a value of any other type keeps what its operand is.
create type document as json;
create table docs (id integer primary key, body json not null, typed document not null, n integer not null, label varchar not null);
