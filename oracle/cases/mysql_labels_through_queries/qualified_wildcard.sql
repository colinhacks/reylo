select d.* from (select a as val from swapped) d cross join (select 0 as val) e
