select d.r from cc, (select case when true then c end as r from (select 1 as id) c) d
