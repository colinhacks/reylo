select x.j from aa o(id, c) cross join lateral (select d.j from (select to_jsonb(c) as j from (select 1 as id) c) d) x
