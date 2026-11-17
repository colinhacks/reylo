select c + c as c1, b + b as b1, substring(b, 1, 2) as b2, cast(b as varbinary(2)) as b3, max(b) over () as b4, min(c) over () as c3 from fx
