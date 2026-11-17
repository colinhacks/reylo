select cast(i as varchar(11)) as s, cast(d as int) as di, try_cast(vc as int) as parsed, convert(date, d2) as day, cast(nv as nvarchar(5)) as cut from everything
