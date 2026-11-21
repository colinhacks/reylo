with recursive plain_table as (select cast(null as signed) as n), r (n) as (select n from plain_table union all select n from r where n < 0) select n from r
