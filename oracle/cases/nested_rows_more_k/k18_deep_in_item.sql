select x.j from aa o(id, c) cross join lateral (select (select (select to_jsonb(c)) from (select 1 as id) c) as j) x
