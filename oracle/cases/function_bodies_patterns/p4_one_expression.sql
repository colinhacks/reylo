select p.id, full_name(p) as name from person p where full_name(p) ilike :needle order by full_name(p)
