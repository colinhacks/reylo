-- A cast through float8 rounds to the double before numeric reads it.
select 0.30000000000000004::float8::numeric as v
