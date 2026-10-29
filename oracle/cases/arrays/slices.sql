-- A colon that opens a subscript is a slice with no lower bound, so `n` is the derived table's
-- column and the statement's one parameter is `id`.
select t.tags[:n] as head, t.tags[ :n] as head2, t.tags[(:i)::int] as one from teams t, (select 2 as n) x where t.id = :id
