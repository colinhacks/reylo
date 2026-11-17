select case when id < 0 then c else 'a' end as c1, case when id < 0 then nc else N'a' end as n1, case when id < 0 then b else 0x01 end as b1 from fx
