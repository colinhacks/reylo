select sum(p.score) over () as total, max(p.note) over (order by p.id) as last_note from posts p
