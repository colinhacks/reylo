drop type floatrange;
create type floatrange as range (subtype = double precision);
create type textrange as range (subtype = text, collation = pg_catalog."C");
