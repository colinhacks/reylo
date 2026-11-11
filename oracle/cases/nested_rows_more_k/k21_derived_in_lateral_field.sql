select x.v from aa o(id, c) cross join lateral (select d.v from (select (c).zip as v from (select 1 as zip) c) d) x
