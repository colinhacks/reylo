-- A bare integer in a grouping set is a position in the select list, never a constant the other columns hold.
select kind, count(*) + 1 as n, coalesce(note, '1') || 1 as tagged from events group by rollup (1), note
