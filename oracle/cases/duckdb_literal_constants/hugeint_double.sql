-- DuckDB's conversion of a HUGEINT to a DOUBLE is a step off the double nearest it.
select 86205717427536125115 as v union all select 1e0
