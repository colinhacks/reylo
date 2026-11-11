select l.one from cc, lateral (select (select c from (select 'z' as street) c) as one) l
