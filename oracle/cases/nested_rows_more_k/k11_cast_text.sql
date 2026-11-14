select x.s from aa o(id, c) cross join lateral (select c::text as s from (select 5 as street) c) x
