insert into req select (r).* from (select row(-424242, 'a', 'b', null)::req as r) s
