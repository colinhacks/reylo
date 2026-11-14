-- DuckDB nests block comments, so the first string is in one.
select /* /* */ 'a' -- */
'b' as v
