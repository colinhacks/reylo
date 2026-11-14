-- A NULL cast to "char" makes every arm a "char", which keeps the first character: no enum.
select 'abc' as v union all select null::"char"
