select max(p.score) over () as top, min(p.note) over (order by p.id) as first_note from posts p
