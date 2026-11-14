-- A NULL cast to name cuts every arm to 63 bytes.
select 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx' as v union all select null::name
