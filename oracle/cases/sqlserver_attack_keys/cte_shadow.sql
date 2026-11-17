with plainpk as (select p.id, p.v from dbo.plainpk p cross join (values (1), (2)) x (n)) select id, v from plainpk where id = :id
