-- A backslash before a or f is the letter, as MySQL reads it.
select '\a' as a, '\f' as f, 'it''s' as q, '\n' as n
