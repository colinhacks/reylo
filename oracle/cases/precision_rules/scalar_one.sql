select (select 1) as n, (select count(*) from member limit 1) as c, (select coalesce(max(id), 0) from member) as m
